pub mod anthropic;
pub mod bedrock;
pub mod convert;
pub mod openai;
pub mod responses;
pub mod types;

pub use anthropic::AnthropicAdapter;
pub use bedrock::BedrockAdapter;
pub use openai::OpenAiAdapter;
pub use responses::ResponsesAdapter;
pub use types::{
    ContentPart, Message, MessageContent, ProviderError, StreamError, StreamResponse, TokenUsage,
    UpstreamRequest, UpstreamResponse,
};

/// Common interface for all LLM provider adapters.
///
/// Implementors receive a `base_url`, `api_key`, and per-model `config` per
/// call so that a single adapter instance can be shared across multiple
/// configured providers.  When `config["endpoint_url"]` is a string the
/// adapter should use it directly instead of constructing a URL from `base_url`.
pub trait ProviderAdapter: Send + Sync {
    /// Send a non-streaming chat completion request and return the full response.
    fn chat_completion(
        &self,
        base_url: &str,
        api_key: &str,
        req: UpstreamRequest,
        config: &serde_json::Value,
    ) -> impl std::future::Future<Output = Result<UpstreamResponse, ProviderError>> + Send;

    /// Send a streaming chat completion request and return a [`StreamResponse`]
    /// that carries the raw byte stream plus a channel for final token usage.
    fn chat_completion_stream(
        &self,
        base_url: &str,
        api_key: &str,
        req: UpstreamRequest,
        config: &serde_json::Value,
    ) -> impl std::future::Future<Output = Result<StreamResponse, ProviderError>> + Send;
}
