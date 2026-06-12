use bytes::Bytes;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tokio::sync::oneshot;
use tokio_stream::Stream;

/// A single content part (for multi-modal messages)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentPart {
    #[serde(rename = "type")]
    pub part_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    // image_url, etc., left as flexible Value
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Message content — either plain text or a list of parts.
/// Deserializes JSON null as empty text.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Parts(Vec<ContentPart>),
}

impl Default for MessageContent {
    fn default() -> Self {
        MessageContent::Text(String::new())
    }
}

/// A single chat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    #[serde(default, deserialize_with = "deserialize_content")]
    pub content: MessageContent,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

fn deserialize_content<'de, D>(deserializer: D) -> Result<MessageContent, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = Option::<Value>::deserialize(deserializer)?;
    match v {
        None | Some(Value::Null) => Ok(MessageContent::default()),
        Some(Value::String(s)) => Ok(MessageContent::Text(s)),
        Some(Value::Array(arr)) => {
            let parts: Vec<ContentPart> = arr.into_iter()
                .filter_map(|item| serde_json::from_value(item).ok())
                .collect();
            Ok(MessageContent::Parts(parts))
        }
        Some(other) => Ok(MessageContent::Text(other.to_string())),
    }
}

/// The canonical upstream request sent to any provider adapter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub stream: bool,
    /// Extra provider-specific fields (forwarded as-is when possible)
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Token usage reported by the provider
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub cache_read_tokens: i32,
    pub cache_write_tokens: i32,
}

/// Non-streaming response from a provider
#[derive(Debug)]
pub struct UpstreamResponse {
    pub status: u16,
    pub body: Value,
    pub usage: TokenUsage,
}

/// Error type for stream chunks
#[derive(Debug, thiserror::Error)]
pub enum StreamError {
    #[error("network error: {0}")]
    Network(String),
    #[error("parse error: {0}")]
    Parse(String),
}

/// Streaming response handle
pub struct StreamResponse {
    /// Raw byte chunks from the provider
    pub stream: Box<dyn Stream<Item = Result<Bytes, StreamError>> + Send + Unpin>,
    /// Receives the final aggregated usage once the stream completes
    pub usage: oneshot::Receiver<TokenUsage>,
}

/// Top-level error for provider operations
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("HTTP error {status}: {body}")]
    Http { status: u16, body: String },
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("stream error: {0}")]
    Stream(String),
}
