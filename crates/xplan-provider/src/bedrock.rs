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

/// Adapter for AWS Bedrock Converse API
pub struct BedrockAdapter {
    client: reqwest::Client,
}

impl BedrockAdapter {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    /// Split messages into system blocks and non-system messages.
    /// OpenAI `role: "tool"` messages are converted to Bedrock `toolResult` blocks.
    fn split_system(messages: &[Message]) -> (Vec<Value>, Vec<Value>) {
        let mut system_blocks: Vec<Value> = Vec::new();
        let mut chat_messages: Vec<Value> = Vec::new();

        for msg in messages {
            if msg.role == "system" {
                match &msg.content {
                    MessageContent::Text(t) => {
                        system_blocks.push(json!({ "text": t }));
                    }
                    MessageContent::Parts(parts) => {
                        for p in parts {
                            if let Some(text) = &p.text {
                                system_blocks.push(json!({ "text": text }));
                            }
                        }
                    }
                }
            } else if msg.role == "tool" {
                // Convert OpenAI tool result message to Bedrock toolResult content block.
                let text = match &msg.content {
                    MessageContent::Text(t) => t.clone(),
                    MessageContent::Parts(parts) => parts
                        .iter()
                        .filter_map(|p| p.text.clone())
                        .collect::<Vec<_>>()
                        .join(""),
                };
                let tool_id = msg.tool_call_id.as_deref().unwrap_or("tool_call_0");
                let synthetic = json!({
                    "role": "tool",
                    "tool_call_id": tool_id,
                    "content": text,
                });
                let block = convert::tool_result_openai_to_bedrock(&synthetic);
                chat_messages.push(json!({
                    "role": "user",
                    "content": [block],
                }));
            } else {
                let mut content = message_content_to_bedrock(&msg.content);
                // If assistant message has tool_calls, convert to Bedrock toolUse blocks
                if msg.role == "assistant" {
                    if let Some(tool_calls) = &msg.tool_calls {
                        if let Some(arr) = tool_calls.as_array() {
                            for tc in arr {
                                let id = tc.get("id").and_then(|v| v.as_str()).unwrap_or("tool_0");
                                let name = tc.get("function").and_then(|f| f.get("name")).and_then(|n| n.as_str()).unwrap_or("");
                                let args_str = tc.get("function").and_then(|f| f.get("arguments")).and_then(|a| a.as_str()).unwrap_or("{}");
                                let input: Value = serde_json::from_str(args_str).unwrap_or(json!({}));
                                content.push(json!({
                                    "toolUse": {
                                        "toolUseId": id,
                                        "name": name,
                                        "input": input,
                                    }
                                }));
                            }
                        }
                    }
                }
                // Remove empty text blocks if we have tool content
                if content.len() > 1 {
                    content.retain(|c| {
                        !(c.get("text").and_then(|t| t.as_str()) == Some(""))
                    });
                }
                if content.is_empty() {
                    content.push(json!({"text": ""}));
                }
                chat_messages.push(json!({
                    "role": msg.role,
                    "content": content,
                }));
            }
        }

        (system_blocks, chat_messages)
    }

    fn build_body(req: &UpstreamRequest, config: &Value) -> Value {
        let (system_blocks, chat_messages) = Self::split_system(&req.messages);

        let mut body = json!({
            "messages": chat_messages,
        });

        if !system_blocks.is_empty() {
            body["system"] = json!(system_blocks);
        }

        let mut inference_config = json!({});
        if let Some(temp) = req.temperature {
            inference_config["temperature"] = json!(temp);
        }
        if let Some(max) = req.max_tokens {
            inference_config["maxTokens"] = json!(max);
        }

        // Map OpenAI `stop` → Bedrock inferenceConfig.stopSequences
        if let Some(stop) = req.extra.get("stop") {
            inference_config["stopSequences"] = stop.clone();
        }

        if inference_config.as_object().map(|m| !m.is_empty()).unwrap_or(false) {
            body["inferenceConfig"] = inference_config;
        }

        // Convert OpenAI response_format → Bedrock native outputConfig
        if let Some(rf) = req.extra.get("response_format") {
            let rf_type = rf.get("type").and_then(|t| t.as_str()).unwrap_or("");
            if rf_type == "json_schema" {
                if let Some(json_schema) = rf.get("json_schema") {
                    let schema = json_schema.get("schema").cloned().unwrap_or(json!({}));
                    let name = json_schema.get("name").and_then(|n| n.as_str()).unwrap_or("structured_output");
                    let description = json_schema.get("description").and_then(|d| d.as_str()).unwrap_or("");
                    // Bedrock expects schema as a JSON string, not an object
                    let schema_string = serde_json::to_string(&schema).unwrap_or_default();
                    body["outputConfig"] = json!({
                        "textFormat": {
                            "type": "json_schema",
                            "structure": {
                                "jsonSchema": {
                                    "schema": schema_string,
                                    "name": name,
                                    "description": description,
                                }
                            }
                        }
                    });
                }
            }
        }

        // Convert tools from OpenAI format → Bedrock toolConfig if present
        // (tools take precedence over response_format tool injection)
        if let Some(tools) = req.extra.get("tools") {
            let mut tool_config = convert::tools_openai_to_bedrock(tools);
            // Optionally add toolChoice
            if let Some(tc) = req.extra.get("tool_choice") {
                if let Some(bedrock_tc) = convert::tool_choice_openai_to_bedrock(tc) {
                    tool_config["toolChoice"] = bedrock_tc;
                }
            }
            body["toolConfig"] = tool_config;
        }

        // Apply per-model parameter modulation from config (Bedrock uses nested paths)
        let paths: &[(&str, &str)] = &[
            ("max_tokens", "inferenceConfig.maxTokens"),
            ("temperature", "inferenceConfig.temperature"),
            ("top_p", "inferenceConfig.topP"),
        ];
        convert::apply_param_modulation(&mut body, config, paths);

        body
    }

    fn extract_usage(body: &Value) -> TokenUsage {
        let usage = &body["usage"];
        TokenUsage {
            input_tokens: usage["inputTokens"].as_i64().unwrap_or(0) as i32,
            output_tokens: usage["outputTokens"].as_i64().unwrap_or(0) as i32,
            cache_read_tokens: usage["cacheReadInputTokenCount"].as_i64().unwrap_or(0) as i32,
            cache_write_tokens: usage["cacheWriteInputTokenCount"].as_i64().unwrap_or(0) as i32,
        }
    }
}

impl Default for BedrockAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl super::ProviderAdapter for BedrockAdapter {
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
            .unwrap_or_else(|| {
                format!(
                    "{}/model/{}/converse",
                    base_url.trim_end_matches('/'),
                    req.model
                )
            });
        let body = Self::build_body(&req, config);

        debug!(url = %url, model = %req.model, "Bedrock non-stream request");

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
            .unwrap_or_else(|| {
                format!(
                    "{}/model/{}/converse-stream",
                    base_url.trim_end_matches('/'),
                    req.model
                )
            });
        let body = Self::build_body(&req, config);

        debug!(url = %url, model = %req.model, "Bedrock stream request");

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
            let mut frame_buf = Vec::<u8>::new();

            while let Some(chunk) = bytes_stream.next().await {
                match chunk {
                    Err(e) => {
                        let _ = tx.send(Err(StreamError::Network(e.to_string()))).await;
                        break;
                    }
                    Ok(bytes) => {
                        frame_buf.extend_from_slice(&bytes);

                        // Parse AWS Event Stream frames from buffer
                        // Frame format: [4 bytes total_len] [4 bytes headers_len] [4 bytes prelude_crc]
                        //               [headers...] [payload...] [4 bytes message_crc]
                        while frame_buf.len() >= 12 {
                            let total_len = u32::from_be_bytes([
                                frame_buf[0], frame_buf[1], frame_buf[2], frame_buf[3],
                            ]) as usize;

                            if frame_buf.len() < total_len {
                                break; // Need more data
                            }

                            let headers_len = u32::from_be_bytes([
                                frame_buf[4], frame_buf[5], frame_buf[6], frame_buf[7],
                            ]) as usize;

                            // Payload starts after prelude (12 bytes) + headers
                            let payload_start = 12 + headers_len;
                            // Payload ends before message CRC (last 4 bytes)
                            let payload_end = total_len - 4;

                            if payload_start <= payload_end {
                                let payload = &frame_buf[payload_start..payload_end];
                                if let Ok(json_str) = std::str::from_utf8(payload) {
                                    if let Ok(v) = serde_json::from_str::<Value>(json_str) {
                                        // Extract usage from metadata event
                                        if let Some(u) = v.get("usage") {
                                            usage.input_tokens =
                                                u["inputTokens"].as_i64().unwrap_or(0) as i32;
                                            usage.output_tokens =
                                                u["outputTokens"].as_i64().unwrap_or(0) as i32;
                                            usage.cache_read_tokens = u
                                                .get("cacheReadInputTokenCount")
                                                .or_else(|| u.get("cacheReadInputTokens"))
                                                .and_then(|x| x.as_i64())
                                                .unwrap_or(0)
                                                as i32;
                                            usage.cache_write_tokens = u
                                                .get("cacheWriteInputTokenCount")
                                                .or_else(|| u.get("cacheWriteInputTokens"))
                                                .and_then(|x| x.as_i64())
                                                .unwrap_or(0)
                                                as i32;
                                        }
                                        if let Some(metrics) = v.get("metrics") {
                                            if let Some(u) = metrics.get("usage").or(v.get("usage")) {
                                                usage.input_tokens =
                                                    u["inputTokens"].as_i64().unwrap_or(usage.input_tokens as i64) as i32;
                                                usage.output_tokens =
                                                    u["outputTokens"].as_i64().unwrap_or(usage.output_tokens as i64) as i32;
                                            }
                                        }

                                        // Forward as SSE text event for downstream processing
                                        let sse_line = format!("data: {}\n\n", v);
                                        if tx.send(Ok(Bytes::from(sse_line))).await.is_err() {
                                            warn!("Bedrock stream receiver dropped");
                                            break;
                                        }
                                    }
                                }
                            }

                            // Remove processed frame from buffer
                            frame_buf.drain(..total_len);
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

fn message_content_to_bedrock(content: &MessageContent) -> Vec<Value> {
    match content {
        MessageContent::Text(text) => vec![json!({ "text": text })],
        MessageContent::Parts(parts) => parts
            .iter()
            .filter_map(|p| {
                if p.part_type == "text" {
                    p.text.as_ref().map(|t| json!({ "text": t }))
                } else {
                    // Other part types (images etc.) — pass through best-effort
                    Some(json!({ "type": p.part_type, "text": p.text }))
                }
            })
            .collect(),
    }
}
