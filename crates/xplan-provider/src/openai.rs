use bytes::Bytes;
use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot};
use tokio_stream::{wrappers::ReceiverStream, StreamExt};
use tracing::{debug, warn};

use crate::convert;
use crate::types::{
    Message, MessageContent, ProviderError, StreamError, StreamResponse, TokenUsage,
    UpstreamRequest, UpstreamResponse,
};

/// Adapter for OpenAI-compatible APIs (OpenAI, Together, Groq, etc.)
pub struct OpenAiAdapter {
    client: reqwest::Client,
}

impl OpenAiAdapter {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    fn build_body(req: &UpstreamRequest, config: &Value) -> Value {
        let messages: Vec<Value> = req
            .messages
            .iter()
            .map(|m| message_to_openai(m))
            .collect();

        let mut body = json!({
            "model": req.model,
            "messages": messages,
            "stream": req.stream,
        });

        if let Some(temp) = req.temperature {
            body["temperature"] = json!(temp);
        }
        if let Some(max) = req.max_tokens {
            body["max_tokens"] = json!(max);
        }

        // Merge extra fields
        if let Value::Object(ref mut map) = body {
            for (k, v) in &req.extra {
                map.insert(k.clone(), v.clone());
            }
        }

        // Degrade json_schema → json_object for providers that don't support strict schema
        // Config: {"structured_output": "json_object_only"}
        let json_object_only = config
            .get("structured_output")
            .and_then(|v| v.as_str())
            == Some("json_object_only");

        if json_object_only {
            if let Some(rf) = body.get("response_format").cloned() {
                if rf.get("type").and_then(|t| t.as_str()) == Some("json_schema") {
                    // Extract schema and inject into system prompt
                    if let Some(schema) = rf.get("json_schema").and_then(|js| js.get("schema")) {
                        let schema_instruction = format!(
                            "You must respond with valid JSON that strictly follows this schema:\n```json\n{}\n```\nDo not include any text outside the JSON.",
                            serde_json::to_string_pretty(schema).unwrap_or_default()
                        );
                        // Prepend schema instruction to messages as system message
                        if let Some(msgs) = body.get_mut("messages").and_then(|m| m.as_array_mut()) {
                            // Check if first message is already system
                            if msgs.first().and_then(|m| m.get("role")).and_then(|r| r.as_str()) == Some("system") {
                                // Append to existing system message
                                if let Some(first) = msgs.first_mut() {
                                    let existing = first.get("content").and_then(|c| c.as_str()).unwrap_or("");
                                    first["content"] = json!(format!("{}\n\n{}", existing, schema_instruction));
                                }
                            } else {
                                msgs.insert(0, json!({"role": "system", "content": schema_instruction}));
                            }
                        }
                    }
                    // Downgrade to json_object
                    body["response_format"] = json!({"type": "json_object"});
                }
            }
        }

        // Apply per-model parameter modulation from config
        let paths: &[(&str, &str)] = &[
            ("max_tokens", "max_tokens"),
            ("temperature", "temperature"),
            ("top_p", "top_p"),
            ("top_k", "top_k"),
            ("frequency_penalty", "frequency_penalty"),
            ("presence_penalty", "presence_penalty"),
        ];
        convert::apply_param_modulation(&mut body, config, paths);

        body
    }

    fn extract_usage(body: &Value) -> TokenUsage {
        let usage = &body["usage"];
        let input_tokens = usage["prompt_tokens"].as_i64().unwrap_or(0) as i32;
        let output_tokens = usage["completion_tokens"].as_i64().unwrap_or(0) as i32;
        let cache_read_tokens = body["usage"]["prompt_tokens_details"]["cached_tokens"]
            .as_i64()
            .unwrap_or(0) as i32;
        TokenUsage {
            input_tokens,
            output_tokens,
            cache_read_tokens,
            cache_write_tokens: 0,
        }
    }
}

impl Default for OpenAiAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl super::ProviderAdapter for OpenAiAdapter {
    async fn chat_completion(
        &self,
        base_url: &str,
        api_key: &str,
        req: UpstreamRequest,
        config: &serde_json::Value,
    ) -> Result<UpstreamResponse, ProviderError> {
        let url = config
            .get("endpoint_url")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("{}/chat/completions", base_url.trim_end_matches('/')));
        let body = Self::build_body(&req, config);

        debug!(url = %url, model = %req.model, "OpenAI non-stream request");

        let resp = self
            .client
            .post(&url)
            .bearer_auth(api_key)
            .json(&body)
            .send()
            .await?;

        let status = resp.status().as_u16();
        let text = resp.text().await?;
        let json_body: Value = serde_json::from_str(&text)
            .map_err(|e| ProviderError::Parse(format!("invalid JSON: {e}: {text}")))?;

        if status >= 400 {
            return Err(ProviderError::Http {
                status,
                body: text,
            });
        }

        let usage = Self::extract_usage(&json_body);
        Ok(UpstreamResponse {
            status,
            body: json_body,
            usage,
        })
    }

    async fn chat_completion_stream(
        &self,
        base_url: &str,
        api_key: &str,
        req: UpstreamRequest,
        config: &serde_json::Value,
    ) -> Result<StreamResponse, ProviderError> {
        let url = config
            .get("endpoint_url")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("{}/chat/completions", base_url.trim_end_matches('/')));
        let mut body = Self::build_body(&req, config);
        body["stream"] = json!(true);
        // Request usage in stream response
        body["stream_options"] = json!({"include_usage": true});

        debug!(url = %url, model = %req.model, "OpenAI stream request");

        let resp = self
            .client
            .post(&url)
            .bearer_auth(api_key)
            .json(&body)
            .send()
            .await?;

        let status = resp.status().as_u16();
        if status >= 400 {
            let text = resp.text().await?;
            return Err(ProviderError::Http { status, body: text });
        }

        let (tx, rx) = mpsc::channel::<Result<Bytes, StreamError>>(64);
        let (usage_tx, usage_rx) = oneshot::channel::<TokenUsage>();

        let mut bytes_stream = resp.bytes_stream();

        tokio::spawn(async move {
            let mut usage = TokenUsage::default();
            let mut buf = String::new();

            while let Some(chunk) = bytes_stream.next().await {
                match chunk {
                    Err(e) => {
                        let _ = tx.send(Err(StreamError::Network(e.to_string()))).await;
                        break;
                    }
                    Ok(bytes) => {
                        // Try to parse SSE for usage extraction (best-effort)
                        if let Ok(text) = std::str::from_utf8(&bytes) {
                            buf.push_str(text);
                            // Process complete SSE lines
                            while let Some(pos) = buf.find("\n\n") {
                                let event = buf[..pos].to_string();
                                buf = buf[pos + 2..].to_string();
                                for line in event.lines() {
                                    if let Some(data) = line.strip_prefix("data: ") {
                                        if data == "[DONE]" {
                                            continue;
                                        }
                                        if let Ok(v) = serde_json::from_str::<Value>(data) {
                                            // Extract usage from stream_options response
                                            if let Some(u) = v.get("usage") {
                                                usage.input_tokens = u["prompt_tokens"]
                                                    .as_i64()
                                                    .unwrap_or(0)
                                                    as i32;
                                                usage.output_tokens = u["completion_tokens"]
                                                    .as_i64()
                                                    .unwrap_or(0)
                                                    as i32;
                                                usage.cache_read_tokens = u
                                                    ["prompt_tokens_details"]["cached_tokens"]
                                                    .as_i64()
                                                    .unwrap_or(0)
                                                    as i32;
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        if tx.send(Ok(bytes)).await.is_err() {
                            warn!("OpenAI stream receiver dropped");
                            break;
                        }
                    }
                }
            }

            let _ = usage_tx.send(usage);
        });

        Ok(StreamResponse {
            stream: Box::new(ReceiverStream::new(rx)),
            usage: usage_rx,
        })
    }
}

fn message_to_openai(msg: &Message) -> Value {
    let mut result = match &msg.content {
        MessageContent::Text(text) => {
            if text.is_empty() && msg.tool_calls.is_some() {
                json!({"role": msg.role, "content": Value::Null})
            } else {
                json!({"role": msg.role, "content": text})
            }
        }
        MessageContent::Parts(parts) => {
            let parts_json: Vec<Value> = parts
                .iter()
                .map(|p| {
                    let mut v = json!({ "type": p.part_type });
                    if let Some(text) = &p.text {
                        v["text"] = json!(text);
                    }
                    if let Value::Object(ref mut m) = v {
                        for (k, val) in &p.extra {
                            m.insert(k.clone(), val.clone());
                        }
                    }
                    v
                })
                .collect();
            json!({"role": msg.role, "content": parts_json})
        }
    };

    if let Some(tool_calls) = &msg.tool_calls {
        result["tool_calls"] = tool_calls.clone();
    }
    if let Some(tool_call_id) = &msg.tool_call_id {
        result["tool_call_id"] = json!(tool_call_id);
    }

    // Preserve any extra fields (e.g., reasoning_content, name, etc.)
    if let Value::Object(ref mut map) = result {
        for (k, v) in &msg.extra {
            map.insert(k.clone(), v.clone());
        }
    }

    result
}
