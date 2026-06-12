use serde_json::{json, Value};
use tracing::debug;

use crate::types::{
    Message, MessageContent, ProviderError, StreamResponse, TokenUsage, UpstreamRequest,
    UpstreamResponse,
};

/// Adapter for the OpenAI Responses API format.
///
/// Used by endpoints like AWS Bedrock that expose an OpenAI-compatible
/// Responses API (`POST /responses`) instead of the standard
/// `chat/completions` endpoint.
///
/// Input format (sent to upstream):
/// ```json
/// {
///   "model": "gpt-5",
///   "input": [{"role": "user", "content": "hello"}],
///   "max_output_tokens": 1000
/// }
/// ```
///
/// Output format (received from upstream):
/// ```json
/// {
///   "id": "resp_xxx",
///   "output": [
///     {"type": "message", "role": "assistant",
///      "content": [{"type": "output_text", "text": "..."}]}
///   ],
///   "usage": {"input_tokens": N, "output_tokens": N, "total_tokens": N}
/// }
/// ```
pub struct ResponsesAdapter {
    client: reqwest::Client,
}

impl ResponsesAdapter {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    /// Convert an `UpstreamRequest` (OpenAI chat format) into Responses API body.
    fn build_body(req: &UpstreamRequest, config: &Value) -> Value {
        // Convert messages to Responses API `input` format.
        // Responses API uses the same role/content structure as OpenAI chat.
        let input: Vec<Value> = req
            .messages
            .iter()
            .map(|m| message_to_responses_input(m))
            .collect();

        let mut body = json!({
            "model": req.model,
            "input": input,
        });

        // Responses API uses max_output_tokens (not max_tokens)
        if let Some(max) = req.max_tokens {
            body["max_output_tokens"] = json!(max);
        }

        // Apply max_output_tokens from config (clamp or set)
        if let Some(max_out) = config.get("max_output_tokens").and_then(|v| v.as_u64()) {
            let current = body.get("max_output_tokens").and_then(|v| v.as_u64());
            match current {
                Some(n) if n > max_out => body["max_output_tokens"] = json!(max_out),
                None => body["max_output_tokens"] = json!(max_out),
                _ => {}
            }
        }

        if let Some(temp) = req.temperature {
            body["temperature"] = json!(temp);
        }

        // Merge extra fields (skip stream as Responses API doesn't use it the same way)
        if let Value::Object(ref mut map) = body {
            for (k, v) in &req.extra {
                if k != "stream" {
                    map.insert(k.clone(), v.clone());
                }
            }
        }

        body
    }

    /// Extract usage from a Responses API response body.
    fn extract_usage(body: &Value) -> TokenUsage {
        let usage = &body["usage"];
        TokenUsage {
            input_tokens: usage["input_tokens"].as_i64().unwrap_or(0) as i32,
            output_tokens: usage["output_tokens"].as_i64().unwrap_or(0) as i32,
            cache_read_tokens: usage["cached_tokens"].as_i64().unwrap_or(0) as i32,
            cache_write_tokens: 0,
        }
    }

    /// Convert a Responses API output body into an OpenAI chat completions body.
    pub fn normalize_to_openai(body: &Value, model_name: &str, usage: &TokenUsage) -> Value {
        let id = format!("chatcmpl-{}", uuid::Uuid::new_v4().simple());
        let created = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Extract text from Responses API output array
        let text = body
            .get("output")
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
            .to_string();

        json!({
            "id": id,
            "object": "chat.completion",
            "created": created,
            "model": model_name,
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": text,
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
        })
    }
}

impl Default for ResponsesAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl super::ProviderAdapter for ResponsesAdapter {
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
            .unwrap_or_else(|| format!("{}/responses", base_url.trim_end_matches('/')));

        let body = Self::build_body(&req, config);

        debug!(url = %url, model = %req.model, "Responses API non-stream request");

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
        _base_url: &str,
        _api_key: &str,
        _req: UpstreamRequest,
        _config: &serde_json::Value,
    ) -> Result<StreamResponse, ProviderError> {
        // Streaming is not yet supported for the Responses API adapter.
        Err(ProviderError::Parse(
            "streaming not yet supported for Responses API format".to_string(),
        ))
    }
}

fn message_to_responses_input(msg: &Message) -> Value {
    let content: Value = match &msg.content {
        MessageContent::Text(text) => json!(text),
        MessageContent::Parts(parts) => {
            let parts_json: Vec<Value> = parts
                .iter()
                .map(|p| {
                    let mut v = json!({ "type": p.part_type });
                    if let Some(text) = &p.text {
                        v["text"] = json!(text);
                    }
                    v
                })
                .collect();
            json!(parts_json)
        }
    };

    json!({
        "role": msg.role,
        "content": content,
    })
}
