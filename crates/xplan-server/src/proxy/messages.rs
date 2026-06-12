use std::time::Instant;

use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Extension, Json,
};
use futures_util::StreamExt as FutStreamExt;
use serde_json::{json, Value};
use xplan_cache::CachedClientKey;
use xplan_db::repo_upstream_key::get_upstream_key;
use xplan_db::repo_usage::{insert_usage_log, UsageLogInsert};
use xplan_provider::{convert, Message, MessageContent, ProviderAdapter, TokenUsage, UpstreamRequest};

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

/// Extract usage from a non-streaming Anthropic response body
fn extract_anthropic_usage(body: &Value) -> TokenUsage {
    let usage = &body["usage"];
    TokenUsage {
        input_tokens: usage["input_tokens"].as_i64().unwrap_or(0) as i32,
        output_tokens: usage["output_tokens"].as_i64().unwrap_or(0) as i32,
        cache_read_tokens: usage["cache_read_input_tokens"].as_i64().unwrap_or(0) as i32,
        cache_write_tokens: usage["cache_creation_input_tokens"]
            .as_i64()
            .unwrap_or(0) as i32,
    }
}

/// Convert an Anthropic Messages API request body into an internal `UpstreamRequest`.
///
/// Handles:
/// - `system` field → system message
/// - `tools` field → forwarded in `extra` as OpenAI format (converted from Anthropic)
/// - message content arrays containing `tool_result` blocks → OpenAI `role:"tool"` messages
/// - message content arrays containing `tool_use` blocks → assistant messages with tool_calls in extra
fn anthropic_to_upstream_request(body: &Value, upstream_model_name: &str) -> UpstreamRequest {
    let mut messages = Vec::new();
    let mut extra: std::collections::HashMap<String, Value> = Default::default();

    // Add system as first message if present
    if let Some(sys) = body.get("system").and_then(|s| s.as_str()) {
        messages.push(Message {
            role: "system".into(),
            content: MessageContent::Text(sys.into()),
            tool_calls: None,
            tool_call_id: None,
            extra: Default::default(),
        });
    }

    // Convert tools from Anthropic format → OpenAI format so downstream adapters can handle them
    if let Some(tools) = body.get("tools") {
        extra.insert("tools".to_string(), convert::tools_anthropic_to_openai(tools));
    }

    // Map Anthropic `stop_sequences` → OpenAI `stop` so downstream adapters receive it correctly
    if let Some(stop_sequences) = body.get("stop_sequences") {
        extra.insert("stop".to_string(), stop_sequences.clone());
    }

    // Map Anthropic output_config.format → OpenAI response_format
    if let Some(output_config) = body.get("output_config") {
        if let Some(format) = output_config.get("format") {
            let fmt_type = format.get("type").and_then(|t| t.as_str()).unwrap_or("");
            if fmt_type == "json_schema" {
                if let Some(schema) = format.get("schema") {
                    let openai_rf = json!({
                        "type": "json_schema",
                        "json_schema": {
                            "name": "structured_output",
                            "schema": schema.clone(),
                        }
                    });
                    extra.insert("response_format".to_string(), openai_rf);
                }
            }
        }
    }

    if let Some(tc) = body.get("tool_choice") {
        // Anthropic tool_choice → OpenAI tool_choice (best-effort passthrough for now)
        // Anthropic: {type:"auto"}/{type:"any"}/{type:"tool",name:"x"}
        // OpenAI: "auto"/"required"/{type:"function",function:{name:"x"}}
        let openai_tc = match tc.get("type").and_then(|t| t.as_str()) {
            Some("auto") => Value::String("auto".into()),
            Some("any") => Value::String("required".into()),
            Some("tool") => {
                if let Some(name) = tc.get("name").and_then(|n| n.as_str()) {
                    json!({"type": "function", "function": {"name": name}})
                } else {
                    Value::String("auto".into())
                }
            }
            _ => Value::String("auto".into()),
        };
        extra.insert("tool_choice".to_string(), openai_tc);
    }

    // Convert messages array
    for msg in body["messages"].as_array().unwrap_or(&vec![]) {
        let role = msg["role"].as_str().unwrap_or("user").to_string();

        // Handle content arrays — may contain tool_result or tool_use blocks
        if let Some(content_arr) = msg["content"].as_array() {
            let has_tool_result = content_arr.iter().any(|b| {
                b.get("type").and_then(|t| t.as_str()) == Some("tool_result")
            });
            let has_tool_use = content_arr.iter().any(|b| {
                b.get("type").and_then(|t| t.as_str()) == Some("tool_use")
            });

            if has_tool_result {
                // Convert each tool_result block into a separate OpenAI tool message
                for block in content_arr {
                    if block.get("type").and_then(|t| t.as_str()) == Some("tool_result") {
                        let openai_msg = convert::tool_result_anthropic_to_openai(block);
                        let tool_call_id = openai_msg["tool_call_id"]
                            .as_str()
                            .unwrap_or("")
                            .to_string();
                        let content_text = openai_msg["content"]
                            .as_str()
                            .unwrap_or("")
                            .to_string();
                        let tid = openai_msg.get("tool_call_id").and_then(|v| v.as_str()).map(|s| s.to_string());
                        messages.push(Message {
                            role: "tool".into(),
                            content: MessageContent::Text(content_text),
                            tool_calls: None,
                            tool_call_id: tid,
                            extra: Default::default(),
                        });
                        // NOTE: tool_call_id is lost in this path because Message.content
                        // is a simple string. Downstream adapters will get empty id.
                        // Full fidelity requires a richer message type; this is best-effort.
                        let _ = tool_call_id;
                    }
                }
                continue;
            }

            if has_tool_use {
                // Assistant message with tool_use blocks → convert to OpenAI assistant message
                // with tool_calls; store tool_calls in the message text as JSON for now.
                // The OpenAI adapter will receive this as a regular message.
                let tool_calls = convert::tool_calls_anthropic_to_openai(&msg["content"]);
                let text_parts: Vec<&str> = content_arr
                    .iter()
                    .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("text"))
                    .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                    .collect();
                let text = text_parts.join("");
                // Store tool_calls as JSON string embedded in content — downstream adapters
                // that support tool_calls will need to parse this.
                // For best compatibility, store as text if there are text parts, otherwise empty.
                messages.push(Message {
                    role: role.clone(),
                    content: MessageContent::Text(if text.is_empty() {
                        String::new()
                    } else {
                        text
                    }),
                    tool_calls: Some(tool_calls),
                    tool_call_id: None,
                    extra: Default::default(),
                });
                continue;
            }

            // Regular content array (e.g., text + image) — serialize as string
            let combined: Vec<String> = content_arr
                .iter()
                .filter_map(|b| {
                    if b.get("type").and_then(|t| t.as_str()) == Some("text") {
                        b.get("text").and_then(|t| t.as_str()).map(|s| s.to_string())
                    } else {
                        None
                    }
                })
                .collect();
            messages.push(Message {
                role,
                content: MessageContent::Text(combined.join("")),
                tool_calls: None,
                tool_call_id: None,
                extra: Default::default(),
            });
            continue;
        }

        // Simple string content
        let content = if let Some(text) = msg["content"].as_str() {
            MessageContent::Text(text.into())
        } else {
            MessageContent::Text(msg["content"].to_string())
        };
        messages.push(Message { role, content, tool_calls: None, tool_call_id: None, extra: Default::default() });
    }

    UpstreamRequest {
        model: upstream_model_name.into(),
        messages,
        temperature: body.get("temperature").and_then(|t| t.as_f64()),
        max_tokens: body.get("max_tokens").and_then(|m| m.as_u64()).map(|m| m as u32),
        stream: body.get("stream").and_then(|s| s.as_bool()).unwrap_or(false),
        extra,
    }
}

/// Convert an `UpstreamResponse` from a non-anthropic adapter into Anthropic Messages API format.
fn to_anthropic_response(
    upstream: &xplan_provider::UpstreamResponse,
    api_format: &str,
    model_name: &str,
) -> Value {
    let id = format!("msg_{}", uuid::Uuid::new_v4().simple());
    let usage_obj = json!({
        "input_tokens": upstream.usage.input_tokens,
        "output_tokens": upstream.usage.output_tokens,
        "cache_read_input_tokens": upstream.usage.cache_read_tokens,
        "cache_creation_input_tokens": upstream.usage.cache_write_tokens,
    });

    match api_format {
        "openai" | "openai_compatible" => {
            let choice = upstream.body
                .get("choices")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first());

            let tool_calls = choice
                .and_then(|ch| ch.get("message"))
                .and_then(|m| m.get("tool_calls"));

            if let Some(tc) = tool_calls {
                if tc.is_array() && !tc.as_array().map(|a| a.is_empty()).unwrap_or(true) {
                    let anthropic_content = convert::tool_calls_openai_to_anthropic(tc);
                    return json!({
                        "id": id,
                        "type": "message",
                        "role": "assistant",
                        "model": model_name,
                        "content": anthropic_content,
                        "stop_reason": "tool_use",
                        "usage": usage_obj,
                    });
                }
            }

            let content = extract_text_from_response(&upstream.body, api_format);
            json!({
                "id": id,
                "type": "message",
                "role": "assistant",
                "model": model_name,
                "content": [{"type": "text", "text": content}],
                "stop_reason": "end_turn",
                "usage": usage_obj,
            })
        }
        "bedrock" => {
            let bedrock_content = upstream.body
                .get("output")
                .and_then(|o| o.get("message"))
                .and_then(|m| m.get("content"))
                .cloned()
                .unwrap_or(json!([]));

            if convert::has_bedrock_tool_use(&bedrock_content) {
                let anthropic_content: Vec<Value> = bedrock_content
                    .as_array()
                    .unwrap_or(&vec![])
                    .iter()
                    .filter_map(|block| {
                        if let Some(tu) = block.get("toolUse") {
                            let id_val = tu.get("toolUseId")?.as_str()?.to_string();
                            let name = tu.get("name")?.as_str()?.to_string();
                            Some(json!({
                                "type": "tool_use",
                                "id": id_val,
                                "name": name,
                                "input": tu.get("input").cloned().unwrap_or(json!({})),
                            }))
                        } else if block.get("text").is_some() {
                            Some(json!({
                                "type": "text",
                                "text": block["text"],
                            }))
                        } else {
                            None
                        }
                    })
                    .collect();

                return json!({
                    "id": id,
                    "type": "message",
                    "role": "assistant",
                    "model": model_name,
                    "content": anthropic_content,
                    "stop_reason": "tool_use",
                    "usage": usage_obj,
                });
            }

            let content = extract_text_from_response(&upstream.body, api_format);
            json!({
                "id": id,
                "type": "message",
                "role": "assistant",
                "model": model_name,
                "content": [{"type": "text", "text": content}],
                "stop_reason": "end_turn",
                "usage": usage_obj,
            })
        }
        "responses" => {
            // Responses API output: extract text from output[].content[type=output_text]
            let content = extract_text_from_response(&upstream.body, api_format);
            json!({
                "id": id,
                "type": "message",
                "role": "assistant",
                "model": model_name,
                "content": [{"type": "text", "text": content}],
                "stop_reason": "end_turn",
                "usage": usage_obj,
            })
        }
        _ => {
            let content = extract_text_from_response(&upstream.body, api_format);
            json!({
                "id": id,
                "type": "message",
                "role": "assistant",
                "model": model_name,
                "content": [{"type": "text", "text": content}],
                "stop_reason": "end_turn",
                "usage": usage_obj,
            })
        }
    }
}

/// Extract the assistant text from a provider response body.
fn extract_text_from_response(body: &Value, api_format: &str) -> String {
    match api_format {
        "openai" | "openai_compatible" => {
            // choices[0].message.content
            body.get("choices")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|choice| choice.get("message"))
                .and_then(|msg| msg.get("content"))
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string()
        }
        "bedrock" => {
            // output.message.content[0].text
            body.get("output")
                .and_then(|o| o.get("message"))
                .and_then(|m| m.get("content"))
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|block| block.get("text"))
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string()
        }
        "responses" => {
            // output[type=message].content[type=output_text].text
            body.get("output")
                .and_then(|o| o.as_array())
                .and_then(|arr| {
                    arr.iter().find(|item| {
                        item.get("type").and_then(|t| t.as_str()) == Some("message")
                            && item.get("role").and_then(|r| r.as_str()) == Some("assistant")
                    })
                })
                .and_then(|msg| msg.get("content"))
                .and_then(|content| content.as_array())
                .and_then(|arr| {
                    arr.iter().find(|block| {
                        block.get("type").and_then(|t| t.as_str()) == Some("output_text")
                    })
                })
                .and_then(|block| block.get("text"))
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string()
        }
        _ => "".to_string(),
    }
}

/// Extract text delta from a non-anthropic SSE chunk for Anthropic stream translation.
fn extract_stream_delta_for_anthropic(parsed: &Value, api_format: &str) -> Option<String> {
    match api_format {
        "bedrock" => {
            if parsed.get("contentBlockIndex").is_some() {
                return parsed.get("delta")?.get("text")?.as_str().map(|s| s.to_string());
            }
            if let Some(cbd) = parsed.get("contentBlockDelta") {
                return cbd.get("delta")?.get("text")?.as_str().map(|s| s.to_string());
            }
            None
        }
        _ => {
            // OpenAI: choices[0].delta.content
            parsed
                .get("choices")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|ch| ch.get("delta"))
                .and_then(|d| d.get("content"))
                .and_then(|t| t.as_str())
                .map(|s| s.to_string())
        }
    }
}

/// Check whether an SSE chunk from a non-anthropic stream signals end-of-stream.
fn is_stream_end_for_anthropic(parsed: &Value, api_format: &str) -> bool {
    match api_format {
        "bedrock" => {
            parsed.get("stopReason").is_some()
                || (parsed.get("role").and_then(|r| r.as_str()) == Some("assistant")
                    && parsed.get("contentBlockIndex").is_none()
                    && parsed.get("delta").is_none())
        }
        _ => {
            // OpenAI: choices[0].finish_reason != null
            parsed
                .get("choices")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|ch| ch.get("finish_reason"))
                .map(|fr| !fr.is_null())
                .unwrap_or(false)
        }
    }
}

/// Extract usage tokens from an SSE end chunk for non-anthropic stream.
fn extract_stream_usage_tokens(parsed: &Value, api_format: &str) -> Option<(i64, i64)> {
    match api_format {
        "bedrock" => {
            let usage = parsed.get("usage").or_else(|| {
                parsed.get("metrics").and_then(|m| m.get("usage"))
            })?;
            let input = usage.get("inputTokens").and_then(|v| v.as_i64()).unwrap_or(0);
            let output = usage.get("outputTokens").and_then(|v| v.as_i64()).unwrap_or(0);
            Some((input, output))
        }
        _ => {
            // OpenAI usage object
            let usage = parsed.get("usage")?;
            let input = usage.get("prompt_tokens").and_then(|v| v.as_i64()).unwrap_or(0);
            let output = usage.get("completion_tokens").and_then(|v| v.as_i64()).unwrap_or(0);
            Some((input, output))
        }
    }
}

/// Proxy handler for the Anthropic `/v1/messages` endpoint.
///
/// Accepts Anthropic Messages API format and always returns Anthropic Messages API format,
/// regardless of the upstream provider's native API format.
pub async fn handle_messages(
    State(state): State<AppState>,
    Extension(client_key): Extension<CachedClientKey>,
    headers: HeaderMap,
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

    // 5. Fetch provider name for usage logging (best effort)
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

    // 6. Dispatch based on api_format
    if api_format == "anthropic" {
        // ── Optimised pass-through path: forward raw Anthropic body ──────────

        // Replace model name in body with upstream model name
        if let Some(obj) = body_value.as_object_mut() {
            obj.insert("model".to_string(), json!(upstream_model_name));
        }

        let url = format!("{}/messages", base_url.trim_end_matches('/'));
        let client = reqwest::Client::new();

        let anthropic_version = headers
            .get("anthropic-version")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("2023-06-01")
            .to_string();

        let mut req_builder = client
            .post(&url)
            .header("x-api-key", &api_key)
            .header("anthropic-version", &anthropic_version)
            .header("content-type", "application/json");

        // Forward anthropic-beta header if present
        if let Some(beta) = headers.get("anthropic-beta") {
            if let Ok(beta_str) = beta.to_str() {
                req_builder = req_builder.header("anthropic-beta", beta_str);
            }
        }

        req_builder = req_builder.json(&body_value);

        let start_time = Instant::now();

        let upstream_response = match req_builder.send().await {
            Ok(r) => r,
            Err(e) => {
                tracing::error!(error = %e, "Failed to reach Anthropic upstream");
                return error_response(
                    StatusCode::BAD_GATEWAY,
                    "Failed to reach upstream provider.",
                    "provider_error",
                    "upstream_unreachable",
                );
            }
        };

        let status = upstream_response.status();
        let latency_ms = start_time.elapsed().as_millis() as u32;

        if is_stream {
            let client_key_id = client_key.id;
            let model_name_clone = model_name.clone();
            let provider_name_clone = provider_name.clone();

            state
                .quality()
                .record(provider_model_id, status.is_success(), latency_ms);

            let state_clone = state.clone();
            tokio::spawn(async move {
                let log = UsageLogInsert {
                    client_key_id,
                    upstream_key_id,
                    provider_model_id,
                    model_name: model_name_clone,
                    provider_name: provider_name_clone,
                    input_tokens: 0,
                    output_tokens: 0,
                    cache_read_tokens: 0,
                    cache_write_tokens: 0,
                    cost_cents: 0,
                    latency_ms: latency_ms as i32,
                    ttft_ms: None,
                    status_code: status.as_u16() as i32,
                    is_success: status.is_success(),
                    error_type: if status.is_success() {
                        None
                    } else {
                        Some("provider_error".to_string())
                    },
                };
                if let Err(e) = insert_usage_log(state_clone.pool(), &log).await {
                    tracing::warn!(error = %e, "Failed to insert stream usage log");
                }
            });

            let byte_stream = upstream_response.bytes_stream();
            let body = Body::from_stream(byte_stream.map(
                |r: Result<bytes::Bytes, reqwest::Error>| {
                    r.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
                },
            ));

            let mut response = Response::new(body);
            *response.status_mut() = status;
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
        } else {
            let resp_status = status;
            let resp_bytes = match upstream_response.bytes().await {
                Ok(b) => b,
                Err(e) => {
                    tracing::error!(error = %e, "Failed to read upstream response body");
                    return error_response(
                        StatusCode::BAD_GATEWAY,
                        "Failed to read upstream response.",
                        "provider_error",
                        "upstream_read_error",
                    );
                }
            };

            let resp_value: Value = match serde_json::from_slice(&resp_bytes) {
                Ok(v) => v,
                Err(_) => {
                    state
                        .quality()
                        .record(provider_model_id, false, latency_ms);
                    return (resp_status, axum::body::Bytes::from(resp_bytes)).into_response();
                }
            };

            if resp_status.is_success() {
                let usage = extract_anthropic_usage(&resp_value);
                let cost = state
                    .billing()
                    .calculate_cost(provider_model_id, &usage)
                    .await
                    .unwrap_or(0);

                state.quality().record(provider_model_id, true, latency_ms);

                let pool = state.pool().clone();
                let model_name_clone = model_name.clone();
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
                        status_code: resp_status.as_u16() as i32,
                        is_success: true,
                        error_type: None,
                    };
                    if let Err(e) = insert_usage_log(&pool, &log).await {
                        tracing::warn!(error = %e, "Failed to insert usage log");
                    }
                });

                (resp_status, Json(resp_value)).into_response()
            } else {
                state
                    .quality()
                    .record(provider_model_id, false, latency_ms);

                let pool = state.pool().clone();
                let model_name_clone = model_name.clone();
                let provider_name_clone = provider_name.clone();
                tokio::spawn(async move {
                    let log = UsageLogInsert {
                        client_key_id: client_key.id,
                        upstream_key_id,
                        provider_model_id,
                        model_name: model_name_clone,
                        provider_name: provider_name_clone,
                        input_tokens: 0,
                        output_tokens: 0,
                        cache_read_tokens: 0,
                        cache_write_tokens: 0,
                        cost_cents: 0,
                        latency_ms: latency_ms as i32,
                        ttft_ms: None,
                        status_code: resp_status.as_u16() as i32,
                        is_success: false,
                        error_type: Some("provider_error".to_string()),
                    };
                    if let Err(e) = insert_usage_log(&pool, &log).await {
                        tracing::warn!(error = %e, "Failed to insert failure usage log");
                    }
                });

                (resp_status, Json(resp_value)).into_response()
            }
        }
    } else {
        // ── Non-anthropic path: convert Anthropic → UpstreamRequest, call adapter,
        //   convert response → Anthropic format ───────────────────────────────

        let upstream_req = anthropic_to_upstream_request(&body_value, &upstream_model_name);
        let start_time = Instant::now();

        if is_stream {
            let stream_result = match api_format.as_str() {
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

                    // Transform the upstream SSE stream into Anthropic SSE format
                    let api_format_for_stream = api_format.clone();
                    let model_name_for_stream = model_name.clone();
                    let msg_id = format!("msg_{}", uuid::Uuid::new_v4().simple());
                    let msg_id_clone = msg_id.clone();
                    let model_for_start = model_name_for_stream.clone();

                    // State carried through the stream map closure
                    let mut sent_start = false;
                    let byte_stream = stream_resp.stream.map(
                        move |item| -> Result<axum::body::Bytes, std::io::Error> {
                            match item {
                                Err(e) => Err(std::io::Error::new(
                                    std::io::ErrorKind::Other,
                                    e.to_string(),
                                )),
                                Ok(bytes) => {
                                    let text = String::from_utf8_lossy(&bytes);
                                    let mut output = String::new();

                                    for line in text.lines() {
                                        if let Some(data) = line.strip_prefix("data: ") {
                                            if data == "[DONE]" {
                                                // Emit Anthropic stream termination events
                                                let content_stop = json!({"type": "content_block_stop", "index": 0});
                                                let msg_delta = json!({
                                                    "type": "message_delta",
                                                    "delta": {"stop_reason": "end_turn", "stop_sequence": null},
                                                    "usage": {"output_tokens": 0}
                                                });
                                                let msg_stop = json!({"type": "message_stop"});
                                                output.push_str(&format!(
                                                    "event: content_block_stop\ndata: {}\n\n\
                                                     event: message_delta\ndata: {}\n\n\
                                                     event: message_stop\ndata: {}\n\n",
                                                    content_stop, msg_delta, msg_stop
                                                ));
                                                continue;
                                            }

                                            if let Ok(parsed) =
                                                serde_json::from_str::<Value>(data)
                                            {
                                                // Emit start events on first real chunk
                                                if !sent_start {
                                                    sent_start = true;
                                                    let msg_start = json!({
                                                        "type": "message_start",
                                                        "message": {
                                                            "id": &msg_id_clone,
                                                            "type": "message",
                                                            "role": "assistant",
                                                            "model": &model_for_start,
                                                            "content": [],
                                                            "stop_reason": null,
                                                            "usage": {
                                                                "input_tokens": 0,
                                                                "output_tokens": 0
                                                            }
                                                        }
                                                    });
                                                    let block_start = json!({
                                                        "type": "content_block_start",
                                                        "index": 0,
                                                        "content_block": {
                                                            "type": "text",
                                                            "text": ""
                                                        }
                                                    });
                                                    output.push_str(&format!(
                                                        "event: message_start\ndata: {}\n\n\
                                                         event: content_block_start\ndata: {}\n\n",
                                                        msg_start, block_start
                                                    ));
                                                }

                                                if let Some(text_delta) =
                                                    extract_stream_delta_for_anthropic(
                                                        &parsed,
                                                        &api_format_for_stream,
                                                    )
                                                {
                                                    let delta_event = json!({
                                                        "type": "content_block_delta",
                                                        "index": 0,
                                                        "delta": {
                                                            "type": "text_delta",
                                                            "text": text_delta
                                                        }
                                                    });
                                                    output.push_str(&format!(
                                                        "event: content_block_delta\ndata: {}\n\n",
                                                        delta_event
                                                    ));
                                                } else if is_stream_end_for_anthropic(
                                                    &parsed,
                                                    &api_format_for_stream,
                                                ) {
                                                    // Extract usage if available
                                                    let (input_tok, output_tok) =
                                                        extract_stream_usage_tokens(
                                                            &parsed,
                                                            &api_format_for_stream,
                                                        )
                                                        .unwrap_or((0, 0));

                                                    let content_stop = json!({"type": "content_block_stop", "index": 0});
                                                    let msg_delta = json!({
                                                        "type": "message_delta",
                                                        "delta": {"stop_reason": "end_turn", "stop_sequence": null},
                                                        "usage": {"output_tokens": output_tok}
                                                    });
                                                    let msg_stop = json!({"type": "message_stop"});
                                                    output.push_str(&format!(
                                                        "event: content_block_stop\ndata: {}\n\n\
                                                         event: message_delta\ndata: {}\n\n\
                                                         event: message_stop\ndata: {}\n\n",
                                                        content_stop, msg_delta, msg_stop
                                                    ));
                                                    // Suppress unused variable warning
                                                    let _ = input_tok;
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
            // Non-streaming path for non-anthropic upstreams
            let result = match api_format.as_str() {
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
                        xplan_provider::ProviderError::Http { status, .. } => (
                            StatusCode::from_u16(*status)
                                .unwrap_or(StatusCode::BAD_GATEWAY),
                            e.to_string(),
                        ),
                        _ => (StatusCode::BAD_GATEWAY, e.to_string()),
                    };

                    let pool = state.pool().clone();
                    let model_name_clone = model_name.clone();
                    let provider_name_clone = provider_name.clone();
                    tokio::spawn(async move {
                        let log = UsageLogInsert {
                            client_key_id: client_key.id,
                            upstream_key_id,
                            provider_model_id,
                            model_name: model_name_clone,
                            provider_name: provider_name_clone,
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

                    error_response(status, &err_msg, "provider_error", "upstream_error")
                }
                Ok(upstream_resp) => {
                    let usage = upstream_resp.usage.clone();
                    let cost = state
                        .billing()
                        .calculate_cost(provider_model_id, &usage)
                        .await
                        .unwrap_or(0);

                    state.quality().record(provider_model_id, true, latency_ms);

                    let pool = state.pool().clone();
                    let model_name_clone = model_name.clone();
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

                    let anthropic_body =
                        to_anthropic_response(&upstream_resp, &api_format, &model_name);
                    Json(anthropic_body).into_response()
                }
            }
        }
    }
}
