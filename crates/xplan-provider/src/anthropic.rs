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

/// Adapter for the Anthropic Messages API
pub struct AnthropicAdapter {
    client: reqwest::Client,
}

impl AnthropicAdapter {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    /// Split messages into (system_prompt, non_system_messages)
    fn split_system(messages: &[Message]) -> (Option<String>, Vec<&Message>) {
        let mut system_parts: Vec<String> = Vec::new();
        let mut rest: Vec<&Message> = Vec::new();

        for msg in messages {
            if msg.role == "system" {
                match &msg.content {
                    MessageContent::Text(t) => system_parts.push(t.clone()),
                    MessageContent::Parts(parts) => {
                        for p in parts {
                            if let Some(text) = &p.text {
                                system_parts.push(text.clone());
                            }
                        }
                    }
                }
            } else {
                rest.push(msg);
            }
        }

        let system = if system_parts.is_empty() {
            None
        } else {
            Some(system_parts.join("\n"))
        };
        (system, rest)
    }

    fn message_to_anthropic(msg: &Message) -> Value {
        // OpenAI tool result messages must be wrapped in a user message containing a
        // tool_result content block.
        if msg.role == "tool" {
            let text = match &msg.content {
                MessageContent::Text(t) => t.clone(),
                MessageContent::Parts(parts) => parts
                    .iter()
                    .filter_map(|p| p.text.clone())
                    .collect::<Vec<_>>()
                    .join(""),
            };
            // Build a synthetic OpenAI tool message Value so we can reuse the converter.
            let synthetic = json!({
                "role": "tool",
                "tool_call_id": "",  // caller should pass proper id via extra; best-effort here
                "content": text,
            });
            let block = convert::tool_result_openai_to_anthropic(&synthetic);
            return json!({ "role": "user", "content": [block] });
        }

        match &msg.content {
            MessageContent::Text(text) => json!({
                "role": msg.role,
                "content": text,
            }),
            MessageContent::Parts(parts) => {
                let content: Vec<Value> = parts
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
                json!({ "role": msg.role, "content": content })
            }
        }
    }

    fn build_body(req: &UpstreamRequest, config: &Value) -> Value {
        let (system, rest_msgs) = Self::split_system(&req.messages);
        let messages: Vec<Value> = rest_msgs.iter().map(|m| Self::message_to_anthropic(m)).collect();

        let mut body = json!({
            "model": req.model,
            "messages": messages,
        });

        if let Some(sys) = system {
            body["system"] = json!(sys);
        }
        if let Some(max) = req.max_tokens {
            body["max_tokens"] = json!(max);
        } else {
            // Anthropic requires max_tokens
            body["max_tokens"] = json!(4096);
        }
        if let Some(temp) = req.temperature {
            body["temperature"] = json!(temp);
        }

        // Convert tools from OpenAI format → Anthropic format if present
        let mut extra_without_tool_fields = req.extra.clone();
        if let Some(tools) = extra_without_tool_fields.remove("tools") {
            body["tools"] = convert::tools_openai_to_anthropic(&tools);
        }
        // Convert tool_choice from OpenAI format → Anthropic format if present
        if let Some(tc) = extra_without_tool_fields.remove("tool_choice") {
            if let Some(anthropic_tc) = convert::tool_choice_openai_to_anthropic(&tc) {
                body["tool_choice"] = anthropic_tc;
            }
        }

        // Convert OpenAI response_format → Anthropic structured output
        // Two strategies based on config.structured_output:
        //   - default / unset / "output_config": use Anthropic's native output_config.format
        //   - "tool_call": convert to forced tool call (for Bedrock Anthropic which doesn't support output_config)
        let strategy = config
            .get("structured_output")
            .and_then(|v| v.as_str())
            .unwrap_or("output_config");

        if let Some(rf) = extra_without_tool_fields.remove("response_format") {
            let rf_type = rf.get("type").and_then(|t| t.as_str()).unwrap_or("");
            if rf_type == "json_schema" {
                if let (Some(schema), Some(name)) = (
                    rf.get("json_schema").and_then(|js| js.get("schema")),
                    rf.get("json_schema").and_then(|js| js.get("name")).and_then(|n| n.as_str()),
                ) {
                    if strategy == "tool_call" {
                        // Add a synthetic tool + force it
                        let synthetic_tool = json!({
                            "name": name,
                            "description": "Generate output matching the required JSON schema.",
                            "input_schema": schema.clone(),
                        });
                        let tools_arr = body
                            .get("tools")
                            .and_then(|v| v.as_array())
                            .cloned()
                            .unwrap_or_default();
                        let mut new_tools = tools_arr;
                        new_tools.push(synthetic_tool);
                        body["tools"] = json!(new_tools);
                        body["tool_choice"] = json!({"type": "tool", "name": name});
                    } else {
                        // Native output_config (Anthropic API)
                        body["output_config"] = json!({
                            "format": {
                                "type": "json_schema",
                                "schema": schema.clone(),
                            }
                        });
                    }
                } else if let Some(schema) = rf.get("json_schema").and_then(|js| js.get("schema")) {
                    // No name provided — only native works
                    if strategy != "tool_call" {
                        body["output_config"] = json!({
                            "format": {
                                "type": "json_schema",
                                "schema": schema.clone(),
                            }
                        });
                    }
                }
            } else if rf_type == "json_object" {
                // Anthropic doesn't have a direct json_object mode, skip
            }
        }

        // Map OpenAI `stop` → Anthropic `stop_sequences`
        if let Some(stop) = extra_without_tool_fields.remove("stop") {
            body["stop_sequences"] = stop;
        }

        // Strip OpenAI-only fields that Anthropic API doesn't accept
        const OPENAI_ONLY_FIELDS: &[&str] = &[
            "stream_options",
            "parallel_tool_calls",
            "frequency_penalty",
            "presence_penalty",
            "logit_bias",
            "logprobs",
            "top_logprobs",
            "n",
            "seed",
            "service_tier",
            "user",
        ];
        for f in OPENAI_ONLY_FIELDS {
            extra_without_tool_fields.remove(*f);
        }

        // Merge remaining extra fields (skip fields already handled above)
        if let Value::Object(ref mut map) = body {
            for (k, v) in extra_without_tool_fields {
                map.insert(k, v);
            }
        }

        // Apply per-model parameter modulation from config
        let paths: &[(&str, &str)] = &[
            ("max_tokens", "max_tokens"),
            ("temperature", "temperature"),
            ("top_p", "top_p"),
            ("top_k", "top_k"),
        ];
        convert::apply_param_modulation(&mut body, config, paths);

        body
    }

    fn extract_usage(body: &Value) -> TokenUsage {
        let usage = &body["usage"];
        TokenUsage {
            input_tokens: usage["input_tokens"].as_i64().unwrap_or(0) as i32,
            output_tokens: usage["output_tokens"].as_i64().unwrap_or(0) as i32,
            cache_read_tokens: usage["cache_read_input_tokens"].as_i64().unwrap_or(0) as i32,
            cache_write_tokens: usage["cache_creation_input_tokens"].as_i64().unwrap_or(0) as i32,
        }
    }
}

impl Default for AnthropicAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl super::ProviderAdapter for AnthropicAdapter {
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
            .unwrap_or_else(|| format!("{}/messages", base_url.trim_end_matches('/')));
        let body = Self::build_body(&req, config);

        debug!(url = %url, model = %req.model, "Anthropic non-stream request");

        let resp = self
            .client
            .post(&url)
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await?;

        let status = resp.status().as_u16();
        let text = resp.text().await?;
        let json_body: Value = serde_json::from_str(&text)
            .map_err(|e| ProviderError::Parse(format!("invalid JSON: {e}: {text}")))?;

        if status >= 400 {
            return Err(ProviderError::Http { status, body: text });
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
            .unwrap_or_else(|| format!("{}/messages", base_url.trim_end_matches('/')));
        let mut body = Self::build_body(&req, config);
        body["stream"] = json!(true);

        debug!(url = %url, model = %req.model, "Anthropic stream request");

        let resp = self
            .client
            .post(&url)
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
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
                        if let Ok(text) = std::str::from_utf8(&bytes) {
                            buf.push_str(text);
                            while let Some(pos) = buf.find("\n\n") {
                                let event = buf[..pos].to_string();
                                buf = buf[pos + 2..].to_string();

                                let mut event_type = String::new();
                                let mut data_str = String::new();

                                for line in event.lines() {
                                    if let Some(et) = line.strip_prefix("event: ") {
                                        event_type = et.to_string();
                                    } else if let Some(data) = line.strip_prefix("data: ") {
                                        data_str = data.to_string();
                                    }
                                }

                                if !data_str.is_empty() {
                                    if let Ok(v) = serde_json::from_str::<Value>(&data_str) {
                                        match event_type.as_str() {
                                            "message_start" => {
                                                if let Some(u) = v["message"]["usage"].as_object() {
                                                    usage.input_tokens = u
                                                        .get("input_tokens")
                                                        .and_then(|x| x.as_i64())
                                                        .unwrap_or(0)
                                                        as i32;
                                                    usage.cache_read_tokens = u
                                                        .get("cache_read_input_tokens")
                                                        .and_then(|x| x.as_i64())
                                                        .unwrap_or(0)
                                                        as i32;
                                                    usage.cache_write_tokens = u
                                                        .get("cache_creation_input_tokens")
                                                        .and_then(|x| x.as_i64())
                                                        .unwrap_or(0)
                                                        as i32;
                                                }
                                            }
                                            "message_delta" => {
                                                if let Some(u) = v.get("usage") {
                                                    usage.output_tokens = u["output_tokens"]
                                                        .as_i64()
                                                        .unwrap_or(0)
                                                        as i32;
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                            }
                        }

                        if tx.send(Ok(bytes)).await.is_err() {
                            warn!("Anthropic stream receiver dropped");
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
