use std::time::Instant;

use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Extension, Json,
};
use serde_json::{json, Value};
use xplan_cache::CachedClientKey;
use xplan_db::repo_upstream_key::get_upstream_key;
use xplan_db::repo_usage::{insert_usage_log, UsageLogInsert};
use xplan_provider::{convert, responses::ResponsesAdapter, ProviderAdapter, UpstreamRequest};

use crate::state::AppState;

fn decrypt_key(encrypted: &[u8], key: &[u8]) -> anyhow::Result<String> {
    use aes_gcm::{aead::Aead, Aes256Gcm, KeyInit, Nonce};
    if encrypted.len() < 12 {
        anyhow::bail!("encrypted key too short");
    }
    let cipher = Aes256Gcm::new_from_slice(key)?;
    let nonce = Nonce::from_slice(&encrypted[..12]);
    let plaintext = cipher
        .decrypt(nonce, &encrypted[12..])
        .map_err(|e| anyhow::anyhow!("decrypt failed: {}", e))?;
    Ok(String::from_utf8(plaintext)?)
}

fn error_response(status: StatusCode, message: &str, error_type: &str, code: &str) -> Response {
    (
        status,
        Json(json!({
            "error": {
                "message": message,
                "type": error_type,
                "code": code
            }
        })),
    )
        .into_response()
}

pub async fn handle_chat_completion(
    State(state): State<AppState>,
    Extension(client_key): Extension<CachedClientKey>,
    _headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    // 1. Parse body
    let mut body_value: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                &format!("invalid JSON body: {}", e),
                "invalid_request_error",
                "invalid_json",
            );
        }
    };

    let model_name = match body_value.get("model").and_then(|v| v.as_str()) {
        Some(m) => m.to_string(),
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "missing 'model' field",
                "invalid_request_error",
                "missing_model",
            );
        }
    };

    let is_stream = body_value
        .get("stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // 2. Check model access
    if let Err(e) = state
        .auth()
        .check_model_access(client_key.id, &model_name, client_key.access_all_models)
        .await
    {
        tracing::warn!(client_id = %client_key.id, model = %model_name, error = %e, "Model access denied");
        return error_response(
            StatusCode::FORBIDDEN,
            "Model access not allowed for this API key.",
            "permission_error",
            "model_not_allowed",
        );
    }

    // 3. Route
    let selected = match state.router().select_upstream(&model_name).await {
        Ok(s) => s,
        Err(xplan_core::RouterError::NoRouteFound(m)) => {
            return error_response(
                StatusCode::NOT_FOUND,
                &format!("No route found for model: {}", m),
                "invalid_request_error",
                "model_not_found",
            );
        }
        Err(xplan_core::RouterError::AllUpstreamsExhausted(m)) => {
            return error_response(
                StatusCode::TOO_MANY_REQUESTS,
                &format!("All upstreams exhausted for model: {}", m),
                "rate_limit_error",
                "upstreams_exhausted",
            );
        }
        Err(e) => {
            tracing::error!(error = %e, "Router internal error");
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal routing error.",
                "internal_error",
                "router_error",
            );
        }
    };

    let upstream_key_id = selected.upstream_key_id;
    let provider_model_id = selected.provider_model_id;
    let upstream_model_name = selected.upstream_model_name.clone();
    let base_url = selected.base_url.clone();
    let api_format = selected.api_format.clone();
    let selected_config = selected.config.clone();

    // 4. Fetch and decrypt upstream key
    let upstream_key_row = match get_upstream_key(state.pool(), upstream_key_id).await {
        Ok(Some(k)) => k,
        Ok(None) => {
            tracing::error!(upstream_key_id = %upstream_key_id, "Upstream key not found in DB");
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Upstream key configuration error.",
                "internal_error",
                "key_not_found",
            );
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to fetch upstream key");
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to fetch upstream key.",
                "internal_error",
                "key_fetch_error",
            );
        }
    };

    let api_key = match decrypt_key(&upstream_key_row.api_key_encrypted, state.encryption_key()) {
        Ok(k) => k,
        Err(e) => {
            tracing::error!(error = %e, "Failed to decrypt upstream key");
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to decrypt upstream key.",
                "internal_error",
                "key_decrypt_error",
            );
        }
    };

    // 5. Replace model name in body
    if let Some(obj) = body_value.as_object_mut() {
        obj.insert("model".to_string(), json!(upstream_model_name));
    }

    // 6. Deserialize into UpstreamRequest
    let upstream_req: UpstreamRequest = match serde_json::from_value(body_value) {
        Ok(r) => r,
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                &format!("invalid request structure: {}", e),
                "invalid_request_error",
                "invalid_request",
            );
        }
    };

    let start_time = Instant::now();

    // 7. Dispatch based on stream/non-stream and api_format
    if is_stream {
        // --- Streaming path ---
        let stream_result = match api_format.as_str() {
            "anthropic" => {
                state
                    .anthropic_adapter()
                    .chat_completion_stream(&base_url, &api_key, upstream_req, &selected_config)
                    .await
            }
            "bedrock" => {
                state
                    .bedrock_adapter()
                    .chat_completion_stream(&base_url, &api_key, upstream_req, &selected_config)
                    .await
            }
            "responses" => {
                state
                    .responses_adapter()
                    .chat_completion_stream(&base_url, &api_key, upstream_req, &selected_config)
                    .await
            }
            _ => {
                state
                    .openai_adapter()
                    .chat_completion_stream(&base_url, &api_key, upstream_req, &selected_config)
                    .await
            }
        };

        match stream_result {
            Err(e) => {
                let latency_ms = start_time.elapsed().as_millis() as u32;
                state
                    .quality()
                    .record(provider_model_id, false, latency_ms);

                tracing::error!(error = %e, "Provider stream error");
                error_response(
                    StatusCode::BAD_GATEWAY,
                    "Upstream provider error.",
                    "provider_error",
                    "upstream_error",
                )
            }
            Ok(stream_resp) => {
                let latency_ms = start_time.elapsed().as_millis() as u32;

                // Get provider name for logging - best effort
                let provider_name = {
                    let pn: Option<(String,)> = sqlx::query_as(
                        "SELECT name FROM providers WHERE id = \
                         (SELECT provider_id FROM upstream_keys WHERE id = $1)",
                    )
                    .bind(upstream_key_id)
                    .fetch_optional(state.pool())
                    .await
                    .unwrap_or(None);
                    pn.map(|(n,)| n).unwrap_or_else(|| api_format.clone())
                };

                // Spawn task to collect usage and record after stream completes
                let state_clone = state.clone();
                let client_key_id = client_key.id;
                let model_name_clone = model_name.clone();
                let provider_name_clone = provider_name.clone();

                let usage_rx = stream_resp.usage;
                tokio::spawn(async move {
                    let usage = usage_rx.await.unwrap_or_default();
                    let cost = state_clone
                        .billing()
                        .calculate_cost(provider_model_id, &usage)
                        .await
                        .unwrap_or(0);
                    state_clone
                        .quality()
                        .record(provider_model_id, true, latency_ms);
                    let log = UsageLogInsert {
                        client_key_id,
                        upstream_key_id,
                        provider_model_id,
                        model_name: model_name_clone,
                        provider_name: provider_name_clone,
                        input_tokens: usage.input_tokens,
                        output_tokens: usage.output_tokens,
                        cache_read_tokens: usage.cache_read_tokens,
                        cache_write_tokens: usage.cache_write_tokens,
                        cost_cents: cost,
                        latency_ms: latency_ms as i32,
                        ttft_ms: None,
                        status_code: 200,
                        is_success: true,
                        error_type: None,
                    };
                    if let Err(e) = insert_usage_log(state_clone.pool(), &log).await {
                        tracing::warn!(error = %e, "Failed to insert usage log for stream");
                    }
                });

                // Build SSE response — transform upstream SSE to OpenAI format
                use futures_util::StreamExt as FutStreamExt;
                let api_format_for_stream = api_format.clone();
                let model_name_for_stream = model_name.clone();
                let stream_id = format!("chatcmpl-{}", uuid::Uuid::new_v4().simple());

                let byte_stream = stream_resp.stream.map(
                    move |item| -> Result<axum::body::Bytes, std::io::Error> {
                        match item {
                            Err(e) => Err(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())),
                            Ok(bytes) => {
                                if api_format_for_stream == "openai_compatible" {
                                    return Ok(bytes);
                                }
                                let text = String::from_utf8_lossy(&bytes);
                                let mut output = String::new();
                                for line in text.lines() {
                                    if let Some(data) = line.strip_prefix("data: ") {
                                        if data == "[DONE]" {
                                            output.push_str("data: [DONE]\n\n");
                                            continue;
                                        }
                                        if let Ok(parsed) = serde_json::from_str::<Value>(data) {
                                            if let Some(delta_text) = extract_stream_delta(&parsed, &api_format_for_stream) {
                                                let chunk = json!({
                                                    "id": &stream_id,
                                                    "object": "chat.completion.chunk",
                                                    "model": &model_name_for_stream,
                                                    "choices": [{
                                                        "index": 0,
                                                        "delta": { "content": delta_text },
                                                        "finish_reason": Value::Null,
                                                    }],
                                                });
                                                output.push_str(&format!("data: {}\n\n", chunk));
                                            } else if is_stream_end(&parsed, &api_format_for_stream) {
                                                let usage = extract_stream_usage(&parsed, &api_format_for_stream);
                                                let mut chunk = json!({
                                                    "id": &stream_id,
                                                    "object": "chat.completion.chunk",
                                                    "model": &model_name_for_stream,
                                                    "choices": [{
                                                        "index": 0,
                                                        "delta": {},
                                                        "finish_reason": "stop",
                                                    }],
                                                });
                                                if let Some(u) = usage {
                                                    chunk["usage"] = u;
                                                }
                                                output.push_str(&format!("data: {}\n\ndata: [DONE]\n\n", chunk));
                                            }
                                        }
                                    }
                                }
                                Ok(axum::body::Bytes::from(output))
                            }
                        }
                    },
                );
                let body = Body::from_stream(byte_stream);
                let mut response = Response::new(body);
                response.headers_mut().insert(
                    "content-type",
                    HeaderValue::from_static("text/event-stream"),
                );
                response.headers_mut().insert(
                    "cache-control",
                    HeaderValue::from_static("no-cache"),
                );
                response.headers_mut().insert(
                    "x-accel-buffering",
                    HeaderValue::from_static("no"),
                );
                response
            }
        }
    } else {
        // --- Non-streaming path ---
        let result = match api_format.as_str() {
            "anthropic" => {
                state
                    .anthropic_adapter()
                    .chat_completion(&base_url, &api_key, upstream_req, &selected_config)
                    .await
            }
            "bedrock" => {
                state
                    .bedrock_adapter()
                    .chat_completion(&base_url, &api_key, upstream_req, &selected_config)
                    .await
            }
            "responses" => {
                state
                    .responses_adapter()
                    .chat_completion(&base_url, &api_key, upstream_req, &selected_config)
                    .await
            }
            _ => {
                state
                    .openai_adapter()
                    .chat_completion(&base_url, &api_key, upstream_req, &selected_config)
                    .await
            }
        };

        let latency_ms = start_time.elapsed().as_millis() as u32;

        match result {
            Err(e) => {
                state
                    .quality()
                    .record(provider_model_id, false, latency_ms);
                tracing::error!(error = %e, "Provider error");

                let (status, err_msg) = match &e {
                    xplan_provider::ProviderError::Http { status, .. } => {
                        (StatusCode::from_u16(*status).unwrap_or(StatusCode::BAD_GATEWAY), e.to_string())
                    }
                    _ => (StatusCode::BAD_GATEWAY, e.to_string()),
                };

                // Record failure usage
                let pool = state.pool().clone();
                let model_name_clone = model_name.clone();
                let provider_name: String = {
                    let pn: Option<(String,)> = sqlx::query_as(
                        "SELECT name FROM providers WHERE id = \
                         (SELECT provider_id FROM upstream_keys WHERE id = $1)",
                    )
                    .bind(upstream_key_id)
                    .fetch_optional(state.pool())
                    .await
                    .unwrap_or(None);
                    pn.map(|(n,)| n).unwrap_or_else(|| api_format.clone())
                };
                tokio::spawn(async move {
                    let log = UsageLogInsert {
                        client_key_id: client_key.id,
                        upstream_key_id,
                        provider_model_id,
                        model_name: model_name_clone,
                        provider_name,
                        input_tokens: 0,
                        output_tokens: 0,
                        cache_read_tokens: 0,
                        cache_write_tokens: 0,
                        cost_cents: 0,
                        latency_ms: latency_ms as i32,
                        ttft_ms: None,
                        status_code: status.as_u16() as i32,
                        is_success: false,
                        error_type: Some("provider_error".to_string()),
                    };
                    if let Err(e) = insert_usage_log(&pool, &log).await {
                        tracing::warn!(error = %e, "Failed to insert failure usage log");
                    }
                });

                error_response(
                    status,
                    &err_msg,
                    "provider_error",
                    "upstream_error",
                )
            }
            Ok(upstream_resp) => {
                // Calculate cost, record quality, log usage
                let usage = upstream_resp.usage.clone();
                let cost = state
                    .billing()
                    .calculate_cost(provider_model_id, &usage)
                    .await
                    .unwrap_or(0);

                state.quality().record(provider_model_id, true, latency_ms);

                let pool = state.pool().clone();
                let model_name_clone = model_name.clone();

                let provider_name = {
                    let pn: Option<(String,)> = sqlx::query_as(
                        "SELECT name FROM providers WHERE id = \
                         (SELECT provider_id FROM upstream_keys WHERE id = $1)",
                    )
                    .bind(upstream_key_id)
                    .fetch_optional(state.pool())
                    .await
                    .unwrap_or(None);
                    pn.map(|(n,)| n).unwrap_or_else(|| api_format.clone())
                };

                let provider_name_clone = provider_name.clone();
                tokio::spawn(async move {
                    let log = UsageLogInsert {
                        client_key_id: client_key.id,
                        upstream_key_id,
                        provider_model_id,
                        model_name: model_name_clone,
                        provider_name: provider_name_clone,
                        input_tokens: usage.input_tokens,
                        output_tokens: usage.output_tokens,
                        cache_read_tokens: usage.cache_read_tokens,
                        cache_write_tokens: usage.cache_write_tokens,
                        cost_cents: cost,
                        latency_ms: latency_ms as i32,
                        ttft_ms: None,
                        status_code: upstream_resp.status as i32,
                        is_success: true,
                        error_type: None,
                    };
                    if let Err(e) = insert_usage_log(&pool, &log).await {
                        tracing::warn!(error = %e, "Failed to insert usage log");
                    }
                });

                let openai_body = normalize_to_openai_format(
                    &upstream_resp.body,
                    &api_format,
                    &model_name,
                    &upstream_resp.usage,
                );
                Json(openai_body).into_response()
            }
        }
    }
}

fn extract_stream_usage(parsed: &Value, api_format: &str) -> Option<Value> {
    let usage = match api_format {
        "anthropic" => {
            // message_delta event has usage, or message_stop with usage
            if parsed.get("type")?.as_str()? == "message_delta" {
                parsed.get("usage")
            } else {
                None
            }
        }
        "bedrock" => {
            // metadata event: {"metrics":...,"usage":{"inputTokens":...,"outputTokens":...}}
            parsed.get("usage").or_else(|| parsed.get("metrics").and_then(|m| m.get("usage")))
        }
        _ => None,
    }?;

    let input = usage.get("inputTokens").or(usage.get("input_tokens"))
        .and_then(|v| v.as_i64()).unwrap_or(0);
    let output = usage.get("outputTokens").or(usage.get("output_tokens"))
        .and_then(|v| v.as_i64()).unwrap_or(0);

    Some(json!({
        "prompt_tokens": input,
        "completion_tokens": output,
        "total_tokens": input + output,
    }))
}

fn is_stream_end(parsed: &Value, api_format: &str) -> bool {
    match api_format {
        "anthropic" => {
            parsed.get("type").and_then(|t| t.as_str()) == Some("message_stop")
        }
        "bedrock" => {
            // Only stopReason is a reliable end signal
            parsed.get("stopReason").is_some()
        }
        _ => false,
    }
}

fn extract_stream_delta(parsed: &Value, api_format: &str) -> Option<String> {
    match api_format {
        "anthropic" => {
            // content_block_delta event: {"type":"content_block_delta","delta":{"type":"text_delta","text":"..."}}
            if parsed.get("type")?.as_str()? == "content_block_delta" {
                return parsed.get("delta")?.get("text")?.as_str().map(|s| s.to_string());
            }
            None
        }
        "bedrock" => {
            // Format from decoded event stream: {"contentBlockIndex":0,"delta":{"text":"..."}}
            if parsed.get("contentBlockIndex").is_some() {
                return parsed.get("delta")?.get("text")?.as_str().map(|s| s.to_string());
            }
            // Alternative: {"contentBlockDelta":{"delta":{"text":"..."}}}
            if let Some(cbd) = parsed.get("contentBlockDelta") {
                return cbd.get("delta")?.get("text")?.as_str().map(|s| s.to_string());
            }
            None
        }
        _ => None,
    }
}

fn normalize_to_openai_format(
    body: &Value,
    api_format: &str,
    model_name: &str,
    usage: &xplan_provider::TokenUsage,
) -> Value {
    if api_format == "openai_compatible" {
        return body.clone();
    }

    if api_format == "responses" {
        return ResponsesAdapter::normalize_to_openai(body, model_name, usage);
    }

    // Build OpenAI-compatible response
    let id = format!("chatcmpl-{}", uuid::Uuid::new_v4().simple());
    let created = chrono::Utc::now().timestamp();

    if api_format == "anthropic" {
        // Anthropic: body.content is an array which may contain text and/or tool_use blocks
        let content_arr = body.get("content").and_then(|c| c.as_array());

        let has_tool_use = content_arr
            .map(|arr| {
                arr.iter()
                    .any(|b| b.get("type").and_then(|t| t.as_str()) == Some("tool_use"))
            })
            .unwrap_or(false);

        if has_tool_use {
            let anthropic_content = body.get("content").cloned().unwrap_or(json!([]));
            let tool_calls = convert::tool_calls_anthropic_to_openai(&anthropic_content);
            // Extract any text content as well
            let text_content: Option<String> = content_arr.and_then(|arr| {
                let texts: Vec<&str> = arr
                    .iter()
                    .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("text"))
                    .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                    .collect();
                if texts.is_empty() {
                    None
                } else {
                    Some(texts.join(""))
                }
            });

            return json!({
                "id": id,
                "object": "chat.completion",
                "created": created,
                "model": model_name,
                "choices": [{
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": text_content,
                        "tool_calls": tool_calls,
                    },
                    "finish_reason": "tool_calls",
                }],
                "usage": {
                    "prompt_tokens": usage.input_tokens,
                    "completion_tokens": usage.output_tokens,
                    "total_tokens": usage.input_tokens + usage.output_tokens,
                    "prompt_tokens_details": {
                        "cached_tokens": usage.cache_read_tokens,
                    },
                },
            });
        }

        // Concatenate text from all "text"-type content blocks
        // (skip thinking, redacted_thinking, and other non-text blocks)
        let content = content_arr
            .map(|arr| {
                arr.iter()
                    .filter(|b| {
                        b.get("type")
                            .and_then(|t| t.as_str())
                            .map(|t| t == "text")
                            .unwrap_or(true) // include blocks without explicit type
                    })
                    .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default();

        return json!({
            "id": id,
            "object": "chat.completion",
            "created": created,
            "model": model_name,
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": content,
                },
                "finish_reason": "stop",
            }],
            "usage": {
                "prompt_tokens": usage.input_tokens,
                "completion_tokens": usage.output_tokens,
                "total_tokens": usage.input_tokens + usage.output_tokens,
                "prompt_tokens_details": {
                    "cached_tokens": usage.cache_read_tokens,
                },
            },
        });
    }

    if api_format == "bedrock" {
        // Bedrock: body.output.message.content[0] may have text or toolUse
        let bedrock_content = body
            .get("output")
            .and_then(|o| o.get("message"))
            .and_then(|m| m.get("content"))
            .cloned()
            .unwrap_or(json!([]));

        let has_tool_use = convert::has_bedrock_tool_use(&bedrock_content);

        if has_tool_use {
            let tool_calls = convert::tool_calls_bedrock_to_openai(&bedrock_content);

            return json!({
                "id": id,
                "object": "chat.completion",
                "created": created,
                "model": model_name,
                "choices": [{
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": null,
                        "tool_calls": tool_calls,
                    },
                    "finish_reason": "tool_calls",
                }],
                "usage": {
                    "prompt_tokens": usage.input_tokens,
                    "completion_tokens": usage.output_tokens,
                    "total_tokens": usage.input_tokens + usage.output_tokens,
                    "prompt_tokens_details": {
                        "cached_tokens": usage.cache_read_tokens,
                    },
                },
            });
        }

        // Concatenate text from all content blocks (skip reasoningContent / thinking blocks)
        let content = bedrock_content
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|block| block.get("text").and_then(|t| t.as_str()))
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default();

        return json!({
            "id": id,
            "object": "chat.completion",
            "created": created,
            "model": model_name,
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": content,
                },
                "finish_reason": "stop",
            }],
            "usage": {
                "prompt_tokens": usage.input_tokens,
                "completion_tokens": usage.output_tokens,
                "total_tokens": usage.input_tokens + usage.output_tokens,
                "prompt_tokens_details": {
                    "cached_tokens": usage.cache_read_tokens,
                },
            },
        });
    }

    body.clone()
}
