use std::sync::Arc;

use xplan_cache::{InvalidationPublisher, LocalCache};
use xplan_core::{AuthService, BillingEngine, QualityMonitor, RouterEngine};
use xplan_db::PgPool;
use xplan_provider::{AnthropicAdapter, BedrockAdapter, OpenAiAdapter, ResponsesAdapter};

#[derive(Clone)]
pub struct AppState {
    pub inner: Arc<AppStateInner>,
}

pub struct AppStateInner {
    pub pool: PgPool,
    pub auth: AuthService,
    pub router: RouterEngine,
    pub billing: BillingEngine,
    pub quality: QualityMonitor,
    pub cache: LocalCache,
    pub invalidation_publisher: InvalidationPublisher,
    pub openai_adapter: OpenAiAdapter,
    pub anthropic_adapter: AnthropicAdapter,
    pub bedrock_adapter: BedrockAdapter,
    pub responses_adapter: ResponsesAdapter,
    pub encryption_key: Vec<u8>,
}

impl AppState {
    pub fn new(inner: AppStateInner) -> Self {
        Self {
            inner: Arc::new(inner),
        }
    }

    pub fn auth(&self) -> &AuthService {
        &self.inner.auth
    }

    pub fn router(&self) -> &RouterEngine {
        &self.inner.router
    }

    pub fn billing(&self) -> &BillingEngine {
        &self.inner.billing
    }

    pub fn quality(&self) -> &QualityMonitor {
        &self.inner.quality
    }

    pub fn pool(&self) -> &PgPool {
        &self.inner.pool
    }

    pub fn cache(&self) -> &LocalCache {
        &self.inner.cache
    }

    pub fn publisher(&self) -> &InvalidationPublisher {
        &self.inner.invalidation_publisher
    }

    pub fn encryption_key(&self) -> &[u8] {
        &self.inner.encryption_key
    }

    pub fn openai_adapter(&self) -> &OpenAiAdapter {
        &self.inner.openai_adapter
    }

    pub fn anthropic_adapter(&self) -> &AnthropicAdapter {
        &self.inner.anthropic_adapter
    }

    pub fn bedrock_adapter(&self) -> &BedrockAdapter {
        &self.inner.bedrock_adapter
    }

    pub fn responses_adapter(&self) -> &ResponsesAdapter {
        &self.inner.responses_adapter
    }
}
