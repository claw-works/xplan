# xplan Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a self-hosted LLM API gateway that aggregates multiple providers behind OpenAI-compatible and Anthropic-native interfaces, with intelligent routing, quota management, billing, and quality monitoring.

**Architecture:** Rust workspace with 5 crates (xplan-server, xplan-core, xplan-provider, xplan-db, xplan-cache). Single binary deployment using axum. PostgreSQL for persistence, Redis for quota counters and cache invalidation, moka for local in-process caching.

**Tech Stack:** Rust 2024, axum, sqlx, reqwest, moka, redis-rs, deadpool-redis, serde, aes-gcm, tokio, config (toml)

---

## File Structure

```
xplan/
├── Cargo.toml                          # Workspace definition
├── config/
│   └── default.toml                    # Default configuration
├── migrations/
│   └── 001_initial.sql                 # All tables for V1
├── crates/
│   ├── xplan-db/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                  # Re-exports
│   │       ├── models.rs              # Rust structs matching DB tables
│   │       ├── repo_provider.rs       # Provider CRUD queries
│   │       ├── repo_upstream_key.rs   # Upstream key CRUD queries
│   │       ├── repo_model.rs          # Model + provider_model queries
│   │       ├── repo_client_key.rs     # Client key queries
│   │       ├── repo_usage.rs          # Usage log insert + query
│   │       └── repo_quality.rs        # Quality stats queries
│   ├── xplan-cache/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                  # Re-exports + CacheLayer struct
│   │       ├── local.rs               # moka-based local cache
│   │       ├── redis_pool.rs          # Redis connection pool setup
│   │       ├── quota.rs               # Redis quota counter operations
│   │       └── invalidation.rs        # Redis pub/sub listener for cache invalidation
│   ├── xplan-provider/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                  # ProviderAdapter trait + re-exports
│   │       ├── types.rs               # UpstreamRequest, UpstreamResponse, TokenUsage
│   │       ├── openai.rs              # OpenAI-compatible adapter (OpenAI, DeepSeek, Gemini, etc.)
│   │       ├── anthropic.rs           # Anthropic adapter
│   │       └── bedrock.rs             # AWS Bedrock Converse API adapter
│   ├── xplan-core/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                  # Re-exports
│   │       ├── router.rs             # Router engine (route selection + failover)
│   │       ├── billing.rs            # Cost calculation
│   │       ├── quality.rs            # Sliding window quality monitor
│   │       └── auth.rs               # Client key authentication
│   └── xplan-server/
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs                # Entry point, server startup
│           ├── config.rs              # Config struct + loading
│           ├── state.rs               # AppState (shared state for handlers)
│           ├── middleware/
│           │   └── auth.rs            # Auth extraction middleware
│           ├── proxy/
│           │   ├── mod.rs             # Proxy router setup
│           │   ├── chat.rs            # POST /v1/chat/completions handler
│           │   ├── messages.rs        # POST /v1/messages handler
│           │   └── models.rs          # GET /v1/models handler
│           └── admin/
│               ├── mod.rs             # Admin router setup
│               ├── providers.rs       # Provider CRUD handlers
│               ├── upstream_keys.rs   # Upstream key handlers
│               ├── models.rs          # Model + provider_model handlers
│               ├── client_keys.rs     # Client key handlers
│               ├── usage.rs           # Usage query handler
│               └── dashboard.rs       # Dashboard aggregation handler
└── frontend/                           # (Task 10 — React SPA, separate phase)
```

---

## Task 1: Workspace Scaffolding + Configuration

**Files:**
- Create: `Cargo.toml` (workspace root)
- Create: `crates/xplan-db/Cargo.toml`, `crates/xplan-db/src/lib.rs`
- Create: `crates/xplan-cache/Cargo.toml`, `crates/xplan-cache/src/lib.rs`
- Create: `crates/xplan-provider/Cargo.toml`, `crates/xplan-provider/src/lib.rs`
- Create: `crates/xplan-core/Cargo.toml`, `crates/xplan-core/src/lib.rs`
- Create: `crates/xplan-server/Cargo.toml`, `crates/xplan-server/src/main.rs`
- Create: `crates/xplan-server/src/config.rs`
- Create: `config/default.toml`

- [ ] **Step 1: Create workspace root Cargo.toml**

```toml
[workspace]
resolver = "2"
members = [
    "crates/xplan-db",
    "crates/xplan-cache",
    "crates/xplan-provider",
    "crates/xplan-core",
    "crates/xplan-server",
]

[workspace.package]
edition = "2024"
version = "0.1.0"
license = "MIT"

[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
anyhow = "1"
thiserror = "2"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
sqlx = { version = "0.8", features = ["runtime-tokio", "tls-rustls", "postgres", "uuid", "chrono", "json"] }
redis = { version = "0.27", features = ["tokio-comp", "connection-manager"] }
deadpool-redis = "0.18"
reqwest = { version = "0.12", features = ["json", "stream"] }
axum = { version = "0.8", features = ["macros"] }
tower = "0.5"
tower-http = { version = "0.6", features = ["cors", "trace", "fs"] }
moka = { version = "0.12", features = ["future"] }
config = "0.14"
aes-gcm = "0.10"
sha2 = "0.10"
hex = "0.4"
rand = "0.8"
```

- [ ] **Step 2: Create each crate's Cargo.toml**

`crates/xplan-db/Cargo.toml`:
```toml
[package]
name = "xplan-db"
edition.workspace = true
version.workspace = true

[dependencies]
sqlx.workspace = true
uuid.workspace = true
chrono.workspace = true
serde.workspace = true
serde_json.workspace = true
anyhow.workspace = true
```

`crates/xplan-cache/Cargo.toml`:
```toml
[package]
name = "xplan-cache"
edition.workspace = true
version.workspace = true

[dependencies]
redis.workspace = true
deadpool-redis.workspace = true
moka.workspace = true
tokio.workspace = true
tracing.workspace = true
anyhow.workspace = true
uuid.workspace = true
serde.workspace = true
serde_json.workspace = true
```

`crates/xplan-provider/Cargo.toml`:
```toml
[package]
name = "xplan-provider"
edition.workspace = true
version.workspace = true

[dependencies]
reqwest.workspace = true
serde.workspace = true
serde_json.workspace = true
tokio.workspace = true
tracing.workspace = true
anyhow.workspace = true
thiserror.workspace = true
uuid.workspace = true
futures-core = "0.3"
bytes = "1"
```

`crates/xplan-core/Cargo.toml`:
```toml
[package]
name = "xplan-core"
edition.workspace = true
version.workspace = true

[dependencies]
xplan-db = { path = "../xplan-db" }
xplan-cache = { path = "../xplan-cache" }
xplan-provider = { path = "../xplan-provider" }
uuid.workspace = true
chrono.workspace = true
tokio.workspace = true
tracing.workspace = true
anyhow.workspace = true
rand.workspace = true
sha2.workspace = true
hex.workspace = true
```

`crates/xplan-server/Cargo.toml`:
```toml
[package]
name = "xplan-server"
edition.workspace = true
version.workspace = true

[dependencies]
xplan-db = { path = "../xplan-db" }
xplan-cache = { path = "../xplan-cache" }
xplan-provider = { path = "../xplan-provider" }
xplan-core = { path = "../xplan-core" }
axum.workspace = true
tower.workspace = true
tower-http.workspace = true
tokio.workspace = true
serde.workspace = true
serde_json.workspace = true
uuid.workspace = true
chrono.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
sqlx.workspace = true
config.workspace = true
anyhow.workspace = true
```

- [ ] **Step 3: Create placeholder lib.rs for each library crate**

Each `crates/xplan-*/src/lib.rs`:
```rust
// Will be populated in subsequent tasks
```

- [ ] **Step 4: Create config module and default config**

`crates/xplan-server/src/config.rs`:
```rust
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub admin: AdminConfig,
    pub encryption: EncryptionConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RedisConfig {
    pub url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AdminConfig {
    pub token: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct EncryptionConfig {
    pub key_hex: String,
}

impl AppConfig {
    pub fn load() -> anyhow::Result<Self> {
        let config = config::Config::builder()
            .add_source(config::File::with_name("config/default"))
            .add_source(config::Environment::with_prefix("XPLAN").separator("__"))
            .build()?;
        Ok(config.try_deserialize()?)
    }
}
```

`config/default.toml`:
```toml
[server]
host = "0.0.0.0"
port = 3000

[database]
url = "postgres://dev:dev@localhost/xplan"
max_connections = 10

[redis]
url = "redis://:dev@localhost:6379"

[admin]
token = "xplan-admin-dev-token"

[encryption]
key_hex = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
```

- [ ] **Step 5: Create main.rs entry point**

`crates/xplan-server/src/main.rs`:
```rust
mod config;

use config::AppConfig;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "xplan=debug,tower_http=debug".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = AppConfig::load()?;
    tracing::info!("Starting xplan on {}:{}", config.server.host, config.server.port);

    let listener = tokio::net::TcpListener::bind(format!("{}:{}", config.server.host, config.server.port)).await?;
    let app = axum::Router::new().route("/health", axum::routing::get(|| async { "ok" }));

    tracing::info!("Listening on {}", listener.local_addr()?);
    axum::serve(listener, app).await?;
    Ok(())
}
```

- [ ] **Step 6: Verify it compiles and runs**

Run: `cargo build`
Expected: successful compilation

Run: `cargo run --bin xplan-server`
Expected: logs showing "Starting xplan on 0.0.0.0:3000" (will fail to connect to DB, that's fine — ctrl-c)

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: scaffold workspace with 5 crates and config"
```

---

## Task 2: Database Schema + Migrations

**Files:**
- Create: `migrations/001_initial.sql`
- Create: `crates/xplan-db/src/models.rs`
- Modify: `crates/xplan-db/src/lib.rs`

- [ ] **Step 1: Create the initial migration**

`migrations/001_initial.sql`:
```sql
-- Enum types
CREATE TYPE api_format AS ENUM ('openai_compatible', 'anthropic', 'bedrock');
CREATE TYPE key_status AS ENUM ('healthy', 'degraded', 'disabled');
CREATE TYPE quota_type AS ENUM ('rpm', 'rpd', 'requests_per_window', 'tokens_per_window');
CREATE TYPE period_type AS ENUM ('hourly', 'daily');

-- Providers
CREATE TABLE providers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL UNIQUE,
    base_url VARCHAR(500) NOT NULL,
    api_format api_format NOT NULL,
    auth_type VARCHAR(50) NOT NULL DEFAULT 'api_key',
    auth_config JSONB NOT NULL DEFAULT '{}',
    is_enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Upstream keys
CREATE TABLE upstream_keys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider_id UUID NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
    alias VARCHAR(200) NOT NULL,
    api_key_encrypted BYTEA NOT NULL,
    is_enabled BOOLEAN NOT NULL DEFAULT true,
    status key_status NOT NULL DEFAULT 'healthy',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_upstream_keys_provider ON upstream_keys(provider_id);

-- Key quotas
CREATE TABLE key_quotas (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    upstream_key_id UUID NOT NULL REFERENCES upstream_keys(id) ON DELETE CASCADE,
    quota_type quota_type NOT NULL,
    limit_value INTEGER NOT NULL,
    window_seconds INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_key_quotas_key ON key_quotas(upstream_key_id);

-- Models
CREATE TABLE models (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(200) NOT NULL UNIQUE,
    is_enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Provider models (many-to-many with pricing)
CREATE TABLE provider_models (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider_id UUID NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
    model_id UUID NOT NULL REFERENCES models(id) ON DELETE CASCADE,
    upstream_model_name VARCHAR(200) NOT NULL,
    input_price_per_mtok INTEGER NOT NULL DEFAULT 0,
    output_price_per_mtok INTEGER NOT NULL DEFAULT 0,
    cache_read_price_per_mtok INTEGER NOT NULL DEFAULT 0,
    cache_write_price_per_mtok INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(provider_id, model_id)
);

CREATE INDEX idx_provider_models_model ON provider_models(model_id);
CREATE INDEX idx_provider_models_provider ON provider_models(provider_id);

-- Key model access
CREATE TABLE key_model_access (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    upstream_key_id UUID NOT NULL REFERENCES upstream_keys(id) ON DELETE CASCADE,
    provider_model_id UUID NOT NULL REFERENCES provider_models(id) ON DELETE CASCADE,
    priority INTEGER NOT NULL DEFAULT 0,
    weight INTEGER NOT NULL DEFAULT 100,
    is_enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(upstream_key_id, provider_model_id)
);

CREATE INDEX idx_key_model_access_pm ON key_model_access(provider_model_id);

-- Client keys
CREATE TABLE client_keys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(200) NOT NULL,
    key_hash VARCHAR(128) NOT NULL UNIQUE,
    key_prefix VARCHAR(20) NOT NULL,
    access_all_models BOOLEAN NOT NULL DEFAULT true,
    is_enabled BOOLEAN NOT NULL DEFAULT true,
    rate_limit_rpm INTEGER,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Client model access
CREATE TABLE client_model_access (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    client_key_id UUID NOT NULL REFERENCES client_keys(id) ON DELETE CASCADE,
    model_id UUID NOT NULL REFERENCES models(id) ON DELETE CASCADE,
    is_enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(client_key_id, model_id)
);

-- Usage logs (partitioned by month)
CREATE TABLE usage_logs (
    id BIGSERIAL,
    client_key_id UUID NOT NULL,
    upstream_key_id UUID NOT NULL,
    provider_model_id UUID NOT NULL,
    model_name VARCHAR(200) NOT NULL,
    provider_name VARCHAR(100) NOT NULL,
    input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    cache_read_tokens INTEGER NOT NULL DEFAULT 0,
    cache_write_tokens INTEGER NOT NULL DEFAULT 0,
    cost_cents INTEGER NOT NULL DEFAULT 0,
    latency_ms INTEGER NOT NULL DEFAULT 0,
    ttft_ms INTEGER,
    status_code INTEGER NOT NULL,
    is_success BOOLEAN NOT NULL,
    error_type VARCHAR(50),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (id, created_at)
) PARTITION BY RANGE (created_at);

-- Create partition for current month
CREATE TABLE usage_logs_default PARTITION OF usage_logs DEFAULT;

CREATE INDEX idx_usage_logs_client ON usage_logs(client_key_id, created_at);
CREATE INDEX idx_usage_logs_model ON usage_logs(model_name, created_at);
CREATE INDEX idx_usage_logs_provider ON usage_logs(provider_name, created_at);

-- Quality stats
CREATE TABLE quality_stats (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider_model_id UUID NOT NULL REFERENCES provider_models(id) ON DELETE CASCADE,
    period_start TIMESTAMPTZ NOT NULL,
    period_type period_type NOT NULL,
    total_requests INTEGER NOT NULL DEFAULT 0,
    success_count INTEGER NOT NULL DEFAULT 0,
    error_count INTEGER NOT NULL DEFAULT 0,
    avg_latency_ms INTEGER NOT NULL DEFAULT 0,
    p95_latency_ms INTEGER NOT NULL DEFAULT 0,
    avg_ttft_ms INTEGER,
    total_tokens BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(provider_model_id, period_start, period_type)
);

-- Updated_at trigger function
CREATE OR REPLACE FUNCTION update_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER providers_updated_at BEFORE UPDATE ON providers FOR EACH ROW EXECUTE FUNCTION update_updated_at();
CREATE TRIGGER upstream_keys_updated_at BEFORE UPDATE ON upstream_keys FOR EACH ROW EXECUTE FUNCTION update_updated_at();
CREATE TRIGGER key_quotas_updated_at BEFORE UPDATE ON key_quotas FOR EACH ROW EXECUTE FUNCTION update_updated_at();
CREATE TRIGGER models_updated_at BEFORE UPDATE ON models FOR EACH ROW EXECUTE FUNCTION update_updated_at();
CREATE TRIGGER provider_models_updated_at BEFORE UPDATE ON provider_models FOR EACH ROW EXECUTE FUNCTION update_updated_at();
CREATE TRIGGER key_model_access_updated_at BEFORE UPDATE ON key_model_access FOR EACH ROW EXECUTE FUNCTION update_updated_at();
CREATE TRIGGER client_keys_updated_at BEFORE UPDATE ON client_keys FOR EACH ROW EXECUTE FUNCTION update_updated_at();
```

- [ ] **Step 2: Create Rust model structs**

`crates/xplan-db/src/models.rs`:
```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Enums matching DB
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "api_format", rename_all = "snake_case")]
pub enum ApiFormat {
    OpenaiCompatible,
    Anthropic,
    Bedrock,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "key_status", rename_all = "snake_case")]
pub enum KeyStatus {
    Healthy,
    Degraded,
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "quota_type", rename_all = "snake_case")]
pub enum QuotaType {
    Rpm,
    Rpd,
    RequestsPerWindow,
    TokensPerWindow,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "period_type", rename_all = "snake_case")]
pub enum PeriodType {
    Hourly,
    Daily,
}

// Table structs
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Provider {
    pub id: Uuid,
    pub name: String,
    pub base_url: String,
    pub api_format: ApiFormat,
    pub auth_type: String,
    pub auth_config: serde_json::Value,
    pub is_enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UpstreamKey {
    pub id: Uuid,
    pub provider_id: Uuid,
    pub alias: String,
    pub api_key_encrypted: Vec<u8>,
    pub is_enabled: bool,
    pub status: KeyStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct KeyQuota {
    pub id: Uuid,
    pub upstream_key_id: Uuid,
    pub quota_type: QuotaType,
    pub limit_value: i32,
    pub window_seconds: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Model {
    pub id: Uuid,
    pub name: String,
    pub is_enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProviderModel {
    pub id: Uuid,
    pub provider_id: Uuid,
    pub model_id: Uuid,
    pub upstream_model_name: String,
    pub input_price_per_mtok: i32,
    pub output_price_per_mtok: i32,
    pub cache_read_price_per_mtok: i32,
    pub cache_write_price_per_mtok: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct KeyModelAccess {
    pub id: Uuid,
    pub upstream_key_id: Uuid,
    pub provider_model_id: Uuid,
    pub priority: i32,
    pub weight: i32,
    pub is_enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ClientKey {
    pub id: Uuid,
    pub name: String,
    pub key_hash: String,
    pub key_prefix: String,
    pub access_all_models: bool,
    pub is_enabled: bool,
    pub rate_limit_rpm: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ClientModelAccess {
    pub id: Uuid,
    pub client_key_id: Uuid,
    pub model_id: Uuid,
    pub is_enabled: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UsageLog {
    pub id: i64,
    pub client_key_id: Uuid,
    pub upstream_key_id: Uuid,
    pub provider_model_id: Uuid,
    pub model_name: String,
    pub provider_name: String,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub cache_read_tokens: i32,
    pub cache_write_tokens: i32,
    pub cost_cents: i32,
    pub latency_ms: i32,
    pub ttft_ms: Option<i32>,
    pub status_code: i32,
    pub is_success: bool,
    pub error_type: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct QualityStat {
    pub id: Uuid,
    pub provider_model_id: Uuid,
    pub period_start: DateTime<Utc>,
    pub period_type: PeriodType,
    pub total_requests: i32,
    pub success_count: i32,
    pub error_count: i32,
    pub avg_latency_ms: i32,
    pub p95_latency_ms: i32,
    pub avg_ttft_ms: Option<i32>,
    pub total_tokens: i64,
    pub updated_at: DateTime<Utc>,
}
```

- [ ] **Step 3: Update xplan-db lib.rs**

`crates/xplan-db/src/lib.rs`:
```rust
pub mod models;

pub use sqlx::PgPool;

pub async fn create_pool(database_url: &str, max_connections: u32) -> anyhow::Result<PgPool> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(max_connections)
        .connect(database_url)
        .await?;
    Ok(pool)
}
```

- [ ] **Step 4: Create database and run migration**

Run:
```bash
createdb -U dev xplan
sqlx database create --database-url "postgres://dev:dev@localhost/xplan"
sqlx migrate run --source migrations --database-url "postgres://dev:dev@localhost/xplan"
```
Expected: migration applied successfully

- [ ] **Step 5: Verify compilation**

Run: `cargo build`
Expected: successful compilation

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: add database schema, migrations, and model structs"
```

---

## Task 3: Cache Layer (moka + Redis)

**Files:**
- Create: `crates/xplan-cache/src/local.rs`
- Create: `crates/xplan-cache/src/redis_pool.rs`
- Create: `crates/xplan-cache/src/quota.rs`
- Create: `crates/xplan-cache/src/invalidation.rs`
- Modify: `crates/xplan-cache/src/lib.rs`

- [ ] **Step 1: Implement Redis connection pool**

`crates/xplan-cache/src/redis_pool.rs`:
```rust
use deadpool_redis::{Config, Pool, Runtime};

pub fn create_redis_pool(url: &str) -> anyhow::Result<Pool> {
    let cfg = Config::from_url(url);
    let pool = cfg.create_pool(Some(Runtime::Tokio1))?;
    Ok(pool)
}
```

- [ ] **Step 2: Implement local moka cache**

`crates/xplan-cache/src/local.rs`:
```rust
use moka::future::Cache;
use std::time::Duration;
use uuid::Uuid;

#[derive(Clone)]
pub struct LocalCache {
    pub client_keys: Cache<String, CachedClientKey>,
    pub route_table: Cache<String, Vec<RouteCandidate>>,
    pub price_table: Cache<Uuid, PriceEntry>,
}

#[derive(Clone, Debug)]
pub struct CachedClientKey {
    pub id: Uuid,
    pub name: String,
    pub access_all_models: bool,
    pub is_enabled: bool,
    pub rate_limit_rpm: Option<i32>,
}

#[derive(Clone, Debug)]
pub struct RouteCandidate {
    pub key_model_access_id: Uuid,
    pub upstream_key_id: Uuid,
    pub provider_model_id: Uuid,
    pub provider_id: Uuid,
    pub upstream_model_name: String,
    pub base_url: String,
    pub api_format: String,
    pub priority: i32,
    pub weight: i32,
}

#[derive(Clone, Debug)]
pub struct PriceEntry {
    pub input_price_per_mtok: i32,
    pub output_price_per_mtok: i32,
    pub cache_read_price_per_mtok: i32,
    pub cache_write_price_per_mtok: i32,
}

impl LocalCache {
    pub fn new() -> Self {
        Self {
            client_keys: Cache::builder()
                .time_to_live(Duration::from_secs(60))
                .max_capacity(10_000)
                .build(),
            route_table: Cache::builder()
                .time_to_live(Duration::from_secs(300))
                .max_capacity(1_000)
                .build(),
            price_table: Cache::builder()
                .time_to_live(Duration::from_secs(300))
                .max_capacity(1_000)
                .build(),
        }
    }

    pub fn invalidate_all(&self) {
        self.client_keys.invalidate_all();
        self.route_table.invalidate_all();
        self.price_table.invalidate_all();
    }

    pub fn invalidate_routes(&self) {
        self.route_table.invalidate_all();
    }

    pub fn invalidate_client_key(&self, key_hash: &str) {
        self.client_keys.invalidate(key_hash);
    }
}
```

- [ ] **Step 3: Implement Redis quota tracker**

`crates/xplan-cache/src/quota.rs`:
```rust
use deadpool_redis::Pool;
use redis::AsyncCommands;
use uuid::Uuid;

pub struct QuotaTracker {
    pool: Pool,
}

impl QuotaTracker {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    pub async fn check_and_increment(
        &self,
        upstream_key_id: Uuid,
        quota_id: Uuid,
        limit: i32,
        window_seconds: i32,
    ) -> anyhow::Result<QuotaCheckResult> {
        let key = format!("xplan:quota:{}:{}", upstream_key_id, quota_id);
        let mut conn = self.pool.get().await?;

        let current: Option<i32> = conn.get(&key).await?;
        let current = current.unwrap_or(0);

        if current >= limit {
            return Ok(QuotaCheckResult::Exhausted { current, limit });
        }

        let new_val: i32 = conn.incr(&key, 1).await?;

        // Set expiry only on first increment (when value becomes 1)
        if new_val == 1 {
            let _: () = conn.expire(&key, window_seconds as i64).await?;
        }

        Ok(QuotaCheckResult::Allowed {
            current: new_val,
            limit,
        })
    }

    pub async fn get_usage(
        &self,
        upstream_key_id: Uuid,
        quota_id: Uuid,
    ) -> anyhow::Result<i32> {
        let key = format!("xplan:quota:{}:{}", upstream_key_id, quota_id);
        let mut conn = self.pool.get().await?;
        let val: Option<i32> = conn.get(&key).await?;
        Ok(val.unwrap_or(0))
    }

    pub async fn get_ttl(
        &self,
        upstream_key_id: Uuid,
        quota_id: Uuid,
    ) -> anyhow::Result<i64> {
        let key = format!("xplan:quota:{}:{}", upstream_key_id, quota_id);
        let mut conn = self.pool.get().await?;
        let ttl: i64 = redis::cmd("TTL").arg(&key).query_async(&mut *conn).await?;
        Ok(ttl)
    }
}

#[derive(Debug)]
pub enum QuotaCheckResult {
    Allowed { current: i32, limit: i32 },
    Exhausted { current: i32, limit: i32 },
}

impl QuotaCheckResult {
    pub fn is_allowed(&self) -> bool {
        matches!(self, QuotaCheckResult::Allowed { .. })
    }
}
```

- [ ] **Step 4: Implement cache invalidation via Redis pub/sub**

`crates/xplan-cache/src/invalidation.rs`:
```rust
use tokio::sync::broadcast;
use tracing;

const CHANNEL: &str = "xplan:cache:invalidate";

#[derive(Debug, Clone)]
pub enum InvalidationEvent {
    Routes,
    ClientKey(String), // key_hash
    Prices,
    All,
}

pub struct InvalidationPublisher {
    pool: deadpool_redis::Pool,
}

impl InvalidationPublisher {
    pub fn new(pool: deadpool_redis::Pool) -> Self {
        Self { pool }
    }

    pub async fn publish(&self, event: &InvalidationEvent) -> anyhow::Result<()> {
        let msg = match event {
            InvalidationEvent::Routes => "routes".to_string(),
            InvalidationEvent::ClientKey(hash) => format!("client_key:{}", hash),
            InvalidationEvent::Prices => "prices".to_string(),
            InvalidationEvent::All => "all".to_string(),
        };
        let mut conn = self.pool.get().await?;
        redis::cmd("PUBLISH")
            .arg(CHANNEL)
            .arg(&msg)
            .query_async::<()>(&mut *conn)
            .await?;
        Ok(())
    }
}

pub fn spawn_invalidation_listener(
    redis_url: String,
    tx: broadcast::Sender<InvalidationEvent>,
) {
    tokio::spawn(async move {
        loop {
            match listen_loop(&redis_url, &tx).await {
                Ok(()) => break,
                Err(e) => {
                    tracing::error!("Cache invalidation listener error: {}, reconnecting...", e);
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }
        }
    });
}

async fn listen_loop(
    redis_url: &str,
    tx: &broadcast::Sender<InvalidationEvent>,
) -> anyhow::Result<()> {
    let client = redis::Client::open(redis_url)?;
    let mut pubsub = client.get_async_pubsub().await?;
    pubsub.subscribe(CHANNEL).await?;

    loop {
        let msg = pubsub.on_message().next().await;
        if let Some(msg) = msg {
            let payload: String = msg.get_payload()?;
            let event = parse_event(&payload);
            let _ = tx.send(event);
        }
    }
}

fn parse_event(payload: &str) -> InvalidationEvent {
    if payload == "routes" {
        InvalidationEvent::Routes
    } else if payload == "prices" {
        InvalidationEvent::Prices
    } else if payload == "all" {
        InvalidationEvent::All
    } else if let Some(hash) = payload.strip_prefix("client_key:") {
        InvalidationEvent::ClientKey(hash.to_string())
    } else {
        InvalidationEvent::All
    }
}

use futures_lite::StreamExt;
```

Wait — `futures_lite` isn't in the dependencies. Let me adjust. The `redis` crate's `on_message()` returns a stream. We need `futures-lite` or `tokio-stream`. Let me use `tokio-stream` instead since we're already on tokio.

Actually, with `redis` 0.27, `get_async_pubsub` returns a `PubSub` that has `on_message()` returning a `Stream`. We just need `futures-core` which is already in `xplan-provider`. Let me add it to xplan-cache's Cargo.toml and adjust the code.

Update `crates/xplan-cache/Cargo.toml` to add:
```toml
futures-lite = "2"
```

The invalidation listener code above uses `futures_lite::StreamExt` for `.next()`. This is correct.

- [ ] **Step 5: Wire up lib.rs**

`crates/xplan-cache/src/lib.rs`:
```rust
pub mod local;
pub mod quota;
pub mod redis_pool;
pub mod invalidation;

pub use local::{LocalCache, CachedClientKey, RouteCandidate, PriceEntry};
pub use quota::{QuotaTracker, QuotaCheckResult};
pub use redis_pool::create_redis_pool;
pub use invalidation::{InvalidationPublisher, InvalidationEvent, spawn_invalidation_listener};
```

- [ ] **Step 6: Verify compilation**

Run: `cargo build`
Expected: successful compilation

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: add cache layer with moka local cache, Redis quota tracker, and pub/sub invalidation"
```

---

## Task 4: Provider Adapters (OpenAI + Anthropic + Bedrock)

**Files:**
- Create: `crates/xplan-provider/src/types.rs`
- Create: `crates/xplan-provider/src/openai.rs`
- Create: `crates/xplan-provider/src/anthropic.rs`
- Create: `crates/xplan-provider/src/bedrock.rs`
- Modify: `crates/xplan-provider/src/lib.rs`

- [ ] **Step 1: Define shared types**

`crates/xplan-provider/src/types.rs`:
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: MessageContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Parts(Vec<ContentPart>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentPart {
    #[serde(rename = "type")]
    pub content_type: String,
    #[serde(flatten)]
    pub data: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct UpstreamResponse {
    pub status: u16,
    pub body: serde_json::Value,
    pub usage: TokenUsage,
}

#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub cache_read_tokens: i32,
    pub cache_write_tokens: i32,
}

#[derive(Debug)]
pub struct StreamResponse {
    pub stream: Box<dyn futures_core::Stream<Item = Result<bytes::Bytes, StreamError>> + Send + Unpin>,
    pub usage: tokio::sync::oneshot::Receiver<TokenUsage>,
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("HTTP error: {status} - {body}")]
    Http { status: u16, body: String },
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Stream error: {0}")]
    Stream(String),
}

#[derive(Debug, thiserror::Error)]
pub enum StreamError {
    #[error("Network: {0}")]
    Network(String),
    #[error("Parse: {0}")]
    Parse(String),
}
```

- [ ] **Step 2: Define the ProviderAdapter trait**

`crates/xplan-provider/src/lib.rs`:
```rust
pub mod types;
pub mod openai;
pub mod anthropic;

pub use types::*;

#[trait_variant::make(Send)]
pub trait ProviderAdapter: Send + Sync {
    async fn chat_completion(
        &self,
        base_url: &str,
        api_key: &str,
        req: UpstreamRequest,
    ) -> Result<UpstreamResponse, ProviderError>;

    async fn chat_completion_stream(
        &self,
        base_url: &str,
        api_key: &str,
        req: UpstreamRequest,
    ) -> Result<StreamResponse, ProviderError>;
}
```

Actually, `trait_variant` is a separate crate. Let's use the simpler approach with `async_trait` or just `async fn` in trait (stable since Rust 1.75). Since we're on Rust 2024, async fn in traits is fine:

```rust
pub mod types;
pub mod openai;
pub mod anthropic;

pub use types::*;

pub trait ProviderAdapter: Send + Sync {
    fn chat_completion(
        &self,
        base_url: &str,
        api_key: &str,
        req: UpstreamRequest,
    ) -> impl std::future::Future<Output = Result<UpstreamResponse, ProviderError>> + Send;

    fn chat_completion_stream(
        &self,
        base_url: &str,
        api_key: &str,
        req: UpstreamRequest,
    ) -> impl std::future::Future<Output = Result<StreamResponse, ProviderError>> + Send;
}
```

- [ ] **Step 3: Implement OpenAI-compatible adapter**

`crates/xplan-provider/src/openai.rs`:
```rust
use crate::{ProviderAdapter, ProviderError, StreamError, StreamResponse, TokenUsage, UpstreamRequest, UpstreamResponse};
use bytes::Bytes;
use futures_core::Stream;
use reqwest::Client;
use std::pin::Pin;
use std::task::{Context, Poll};

pub struct OpenAiAdapter {
    client: Client,
}

impl OpenAiAdapter {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl ProviderAdapter for OpenAiAdapter {
    async fn chat_completion(
        &self,
        base_url: &str,
        api_key: &str,
        req: UpstreamRequest,
    ) -> Result<UpstreamResponse, ProviderError> {
        let url = format!("{}/v1/chat/completions", base_url.trim_end_matches('/'));

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&req)
            .send()
            .await?;

        let status = resp.status().as_u16();
        let body: serde_json::Value = resp.json().await?;

        if status >= 400 {
            return Err(ProviderError::Http {
                status,
                body: body.to_string(),
            });
        }

        let usage = extract_openai_usage(&body);
        Ok(UpstreamResponse { status, body, usage })
    }

    async fn chat_completion_stream(
        &self,
        base_url: &str,
        api_key: &str,
        mut req: UpstreamRequest,
    ) -> Result<StreamResponse, ProviderError> {
        req.stream = Some(true);
        let url = format!("{}/v1/chat/completions", base_url.trim_end_matches('/'));

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&req)
            .send()
            .await?;

        let status = resp.status().as_u16();
        if status >= 400 {
            let body = resp.text().await.unwrap_or_default();
            return Err(ProviderError::Http { status, body });
        }

        let (usage_tx, usage_rx) = tokio::sync::oneshot::channel();
        let byte_stream = resp.bytes_stream();

        let stream = OpenAiStream {
            inner: byte_stream,
            usage_tx: Some(usage_tx),
            accumulated_usage: TokenUsage::default(),
            buffer: String::new(),
        };

        Ok(StreamResponse {
            stream: Box::new(stream),
            usage: usage_rx,
        })
    }
}

fn extract_openai_usage(body: &serde_json::Value) -> TokenUsage {
    let usage = &body["usage"];
    TokenUsage {
        input_tokens: usage["prompt_tokens"].as_i64().unwrap_or(0) as i32,
        output_tokens: usage["completion_tokens"].as_i64().unwrap_or(0) as i32,
        cache_read_tokens: usage["prompt_tokens_details"]["cached_tokens"]
            .as_i64()
            .unwrap_or(0) as i32,
        cache_write_tokens: 0, // OpenAI doesn't report cache writes separately
    }
}

struct OpenAiStream {
    inner: reqwest::Response, // actually bytes_stream, we'll type it properly
    usage_tx: Option<tokio::sync::oneshot::Sender<TokenUsage>>,
    accumulated_usage: TokenUsage,
    buffer: String,
}
```

Hmm, this is getting complex for the stream implementation. Let me simplify by using a channel-based approach that works cleanly:

```rust
use crate::{ProviderAdapter, ProviderError, StreamError, StreamResponse, TokenUsage, UpstreamRequest, UpstreamResponse};
use reqwest::Client;

pub struct OpenAiAdapter {
    client: Client,
}

impl OpenAiAdapter {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl ProviderAdapter for OpenAiAdapter {
    async fn chat_completion(
        &self,
        base_url: &str,
        api_key: &str,
        req: UpstreamRequest,
    ) -> Result<UpstreamResponse, ProviderError> {
        let url = format!("{}/v1/chat/completions", base_url.trim_end_matches('/'));

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&req)
            .send()
            .await?;

        let status = resp.status().as_u16();
        let body: serde_json::Value = resp.json().await?;

        if status >= 400 {
            return Err(ProviderError::Http {
                status,
                body: body.to_string(),
            });
        }

        let usage = extract_openai_usage(&body);
        Ok(UpstreamResponse { status, body, usage })
    }

    async fn chat_completion_stream(
        &self,
        base_url: &str,
        api_key: &str,
        mut req: UpstreamRequest,
    ) -> Result<StreamResponse, ProviderError> {
        req.stream = Some(true);
        let url = format!("{}/v1/chat/completions", base_url.trim_end_matches('/'));

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&req)
            .send()
            .await?;

        let status = resp.status().as_u16();
        if status >= 400 {
            let body = resp.text().await.unwrap_or_default();
            return Err(ProviderError::Http { status, body });
        }

        let (usage_tx, usage_rx) = tokio::sync::oneshot::channel();
        let (chunk_tx, chunk_rx) = tokio::sync::mpsc::channel::<Result<bytes::Bytes, StreamError>>(32);

        // Spawn a task to read the SSE stream, forward chunks, and extract final usage
        let byte_stream = resp;
        tokio::spawn(async move {
            let mut usage = TokenUsage::default();
            let mut stream = byte_stream.bytes_stream();

            use futures_core::StreamExt;
            while let Some(chunk) = StreamExt::next(&mut stream).await {
                match chunk {
                    Ok(bytes) => {
                        // Check for usage in the final SSE event
                        if let Ok(text) = std::str::from_utf8(&bytes) {
                            for line in text.lines() {
                                if let Some(data) = line.strip_prefix("data: ") {
                                    if data == "[DONE]" {
                                        continue;
                                    }
                                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data) {
                                        if let Some(u) = parsed.get("usage") {
                                            usage = extract_openai_usage_from_chunk(u);
                                        }
                                    }
                                }
                            }
                        }
                        if chunk_tx.send(Ok(bytes)).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = chunk_tx.send(Err(StreamError::Network(e.to_string()))).await;
                        break;
                    }
                }
            }
            let _ = usage_tx.send(usage);
        });

        let stream = tokio_stream::wrappers::ReceiverStream::new(chunk_rx);
        Ok(StreamResponse {
            stream: Box::new(stream),
            usage: usage_rx,
        })
    }
}

fn extract_openai_usage(body: &serde_json::Value) -> TokenUsage {
    let usage = &body["usage"];
    TokenUsage {
        input_tokens: usage["prompt_tokens"].as_i64().unwrap_or(0) as i32,
        output_tokens: usage["completion_tokens"].as_i64().unwrap_or(0) as i32,
        cache_read_tokens: usage.get("prompt_tokens_details")
            .and_then(|d| d["cached_tokens"].as_i64())
            .unwrap_or(0) as i32,
        cache_write_tokens: 0,
    }
}

fn extract_openai_usage_from_chunk(usage: &serde_json::Value) -> TokenUsage {
    TokenUsage {
        input_tokens: usage["prompt_tokens"].as_i64().unwrap_or(0) as i32,
        output_tokens: usage["completion_tokens"].as_i64().unwrap_or(0) as i32,
        cache_read_tokens: usage.get("prompt_tokens_details")
            .and_then(|d| d["cached_tokens"].as_i64())
            .unwrap_or(0) as i32,
        cache_write_tokens: 0,
    }
}
```

We need `tokio-stream` in xplan-provider's Cargo.toml. Add:
```toml
tokio-stream = "0.1"
```

- [ ] **Step 4: Implement Anthropic adapter**

`crates/xplan-provider/src/anthropic.rs`:
```rust
use crate::{ProviderAdapter, ProviderError, StreamError, StreamResponse, TokenUsage, UpstreamRequest, UpstreamResponse};
use reqwest::Client;
use serde_json::json;

pub struct AnthropicAdapter {
    client: Client,
}

impl AnthropicAdapter {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl ProviderAdapter for AnthropicAdapter {
    async fn chat_completion(
        &self,
        base_url: &str,
        api_key: &str,
        req: UpstreamRequest,
    ) -> Result<UpstreamResponse, ProviderError> {
        let url = format!("{}/v1/messages", base_url.trim_end_matches('/'));
        let anthropic_body = convert_to_anthropic_format(&req);

        let resp = self
            .client
            .post(&url)
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&anthropic_body)
            .send()
            .await?;

        let status = resp.status().as_u16();
        let body: serde_json::Value = resp.json().await?;

        if status >= 400 {
            return Err(ProviderError::Http {
                status,
                body: body.to_string(),
            });
        }

        let usage = extract_anthropic_usage(&body);
        Ok(UpstreamResponse { status, body, usage })
    }

    async fn chat_completion_stream(
        &self,
        base_url: &str,
        api_key: &str,
        req: UpstreamRequest,
    ) -> Result<StreamResponse, ProviderError> {
        let url = format!("{}/v1/messages", base_url.trim_end_matches('/'));
        let mut anthropic_body = convert_to_anthropic_format(&req);
        anthropic_body["stream"] = json!(true);

        let resp = self
            .client
            .post(&url)
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&anthropic_body)
            .send()
            .await?;

        let status = resp.status().as_u16();
        if status >= 400 {
            let body = resp.text().await.unwrap_or_default();
            return Err(ProviderError::Http { status, body });
        }

        let (usage_tx, usage_rx) = tokio::sync::oneshot::channel();
        let (chunk_tx, chunk_rx) = tokio::sync::mpsc::channel::<Result<bytes::Bytes, StreamError>>(32);

        tokio::spawn(async move {
            let mut usage = TokenUsage::default();
            let mut stream = resp.bytes_stream();

            use futures_core::StreamExt;
            while let Some(chunk) = StreamExt::next(&mut stream).await {
                match chunk {
                    Ok(bytes) => {
                        if let Ok(text) = std::str::from_utf8(&bytes) {
                            for line in text.lines() {
                                if let Some(data) = line.strip_prefix("data: ") {
                                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data) {
                                        if parsed["type"] == "message_delta" {
                                            if let Some(u) = parsed.get("usage") {
                                                usage.output_tokens = u["output_tokens"].as_i64().unwrap_or(0) as i32;
                                            }
                                        } else if parsed["type"] == "message_start" {
                                            if let Some(u) = parsed["message"].get("usage") {
                                                usage.input_tokens = u["input_tokens"].as_i64().unwrap_or(0) as i32;
                                                usage.cache_read_tokens = u["cache_read_input_tokens"].as_i64().unwrap_or(0) as i32;
                                                usage.cache_write_tokens = u["cache_creation_input_tokens"].as_i64().unwrap_or(0) as i32;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        if chunk_tx.send(Ok(bytes)).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = chunk_tx.send(Err(StreamError::Network(e.to_string()))).await;
                        break;
                    }
                }
            }
            let _ = usage_tx.send(usage);
        });

        let stream = tokio_stream::wrappers::ReceiverStream::new(chunk_rx);
        Ok(StreamResponse {
            stream: Box::new(stream),
            usage: usage_rx,
        })
    }
}

fn convert_to_anthropic_format(req: &UpstreamRequest) -> serde_json::Value {
    let mut messages = Vec::new();
    let mut system = None;

    for msg in &req.messages {
        if msg.role == "system" {
            system = Some(match &msg.content {
                crate::types::MessageContent::Text(t) => t.clone(),
                crate::types::MessageContent::Parts(parts) => {
                    serde_json::to_string(parts).unwrap_or_default()
                }
            });
        } else {
            messages.push(json!({
                "role": msg.role,
                "content": match &msg.content {
                    crate::types::MessageContent::Text(t) => json!(t),
                    crate::types::MessageContent::Parts(parts) => json!(parts),
                }
            }));
        }
    }

    let mut body = json!({
        "model": req.model,
        "messages": messages,
        "max_tokens": req.max_tokens.unwrap_or(4096),
    });

    if let Some(sys) = system {
        body["system"] = json!(sys);
    }
    if let Some(temp) = req.temperature {
        body["temperature"] = json!(temp);
    }

    body
}

fn extract_anthropic_usage(body: &serde_json::Value) -> TokenUsage {
    let usage = &body["usage"];
    TokenUsage {
        input_tokens: usage["input_tokens"].as_i64().unwrap_or(0) as i32,
        output_tokens: usage["output_tokens"].as_i64().unwrap_or(0) as i32,
        cache_read_tokens: usage["cache_read_input_tokens"].as_i64().unwrap_or(0) as i32,
        cache_write_tokens: usage["cache_creation_input_tokens"].as_i64().unwrap_or(0) as i32,
    }
}
```

- [ ] **Step 5: Implement Bedrock Converse API adapter**

`crates/xplan-provider/src/bedrock.rs`:
```rust
use crate::{ProviderAdapter, ProviderError, StreamError, StreamResponse, TokenUsage, UpstreamRequest, UpstreamResponse};
use reqwest::Client;
use serde_json::json;

pub struct BedrockConverseAdapter {
    client: Client,
}

impl BedrockConverseAdapter {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl ProviderAdapter for BedrockConverseAdapter {
    async fn chat_completion(
        &self,
        base_url: &str,
        api_key: &str,
        req: UpstreamRequest,
    ) -> Result<UpstreamResponse, ProviderError> {
        let url = format!(
            "{}/model/{}/converse",
            base_url.trim_end_matches('/'),
            req.model
        );

        let body = convert_to_bedrock_format(&req);

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = resp.status().as_u16();
        let resp_body: serde_json::Value = resp.json().await?;

        if status >= 400 {
            return Err(ProviderError::Http {
                status,
                body: resp_body.to_string(),
            });
        }

        let usage = extract_bedrock_usage(&resp_body);
        Ok(UpstreamResponse { status, body: resp_body, usage })
    }

    async fn chat_completion_stream(
        &self,
        base_url: &str,
        api_key: &str,
        req: UpstreamRequest,
    ) -> Result<StreamResponse, ProviderError> {
        let url = format!(
            "{}/model/{}/converse-stream",
            base_url.trim_end_matches('/'),
            req.model
        );

        let body = convert_to_bedrock_format(&req);

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = resp.status().as_u16();
        if status >= 400 {
            let body = resp.text().await.unwrap_or_default();
            return Err(ProviderError::Http { status, body });
        }

        let (usage_tx, usage_rx) = tokio::sync::oneshot::channel();
        let (chunk_tx, chunk_rx) = tokio::sync::mpsc::channel::<Result<bytes::Bytes, StreamError>>(32);

        tokio::spawn(async move {
            let mut usage = TokenUsage::default();
            let mut stream = resp.bytes_stream();

            use futures_core::StreamExt;
            while let Some(chunk) = StreamExt::next(&mut stream).await {
                match chunk {
                    Ok(bytes) => {
                        if let Ok(text) = std::str::from_utf8(&bytes) {
                            for line in text.lines() {
                                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(line) {
                                    if let Some(u) = parsed.get("usage") {
                                        usage.input_tokens = u["inputTokens"].as_i64().unwrap_or(0) as i32;
                                        usage.output_tokens = u["outputTokens"].as_i64().unwrap_or(0) as i32;
                                    }
                                }
                            }
                        }
                        if chunk_tx.send(Ok(bytes)).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = chunk_tx.send(Err(StreamError::Network(e.to_string()))).await;
                        break;
                    }
                }
            }
            let _ = usage_tx.send(usage);
        });

        let stream = tokio_stream::wrappers::ReceiverStream::new(chunk_rx);
        Ok(StreamResponse {
            stream: Box::new(stream),
            usage: usage_rx,
        })
    }
}

fn convert_to_bedrock_format(req: &UpstreamRequest) -> serde_json::Value {
    let mut messages = Vec::new();
    let mut system = Vec::new();

    for msg in &req.messages {
        let content_text = match &msg.content {
            crate::types::MessageContent::Text(t) => t.clone(),
            crate::types::MessageContent::Parts(parts) => {
                serde_json::to_string(parts).unwrap_or_default()
            }
        };

        if msg.role == "system" {
            system.push(json!({"text": content_text}));
        } else {
            messages.push(json!({
                "role": if msg.role == "assistant" { "assistant" } else { "user" },
                "content": [{"text": content_text}]
            }));
        }
    }

    let mut body = json!({
        "messages": messages,
    });

    if !system.is_empty() {
        body["system"] = json!(system);
    }

    if let Some(max_tokens) = req.max_tokens {
        body["inferenceConfig"] = json!({
            "maxTokens": max_tokens,
        });
        if let Some(temp) = req.temperature {
            body["inferenceConfig"]["temperature"] = json!(temp);
        }
    } else if let Some(temp) = req.temperature {
        body["inferenceConfig"] = json!({
            "temperature": temp,
        });
    }

    body
}

fn extract_bedrock_usage(body: &serde_json::Value) -> TokenUsage {
    let usage = &body["usage"];
    TokenUsage {
        input_tokens: usage["inputTokens"].as_i64().unwrap_or(0) as i32,
        output_tokens: usage["outputTokens"].as_i64().unwrap_or(0) as i32,
        cache_read_tokens: usage["cacheReadInputTokenCount"].as_i64().unwrap_or(0) as i32,
        cache_write_tokens: usage["cacheWriteInputTokenCount"].as_i64().unwrap_or(0) as i32,
    }
}
```

Update `crates/xplan-provider/src/lib.rs` to add:
```rust
pub mod bedrock;
```

- [ ] **Step 6: Verify compilation**

Run: `cargo build`
Expected: successful compilation (some warnings about unused code are fine at this stage)

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: add provider adapters for OpenAI-compatible, Anthropic, and Bedrock APIs"
```

---

## Task 5: Core Domain Logic (Auth, Router, Billing, Quality)

**Files:**
- Create: `crates/xplan-core/src/auth.rs`
- Create: `crates/xplan-core/src/router.rs`
- Create: `crates/xplan-core/src/billing.rs`
- Create: `crates/xplan-core/src/quality.rs`
- Modify: `crates/xplan-core/src/lib.rs`

- [ ] **Step 1: Implement client key authentication**

`crates/xplan-core/src/auth.rs`:
```rust
use sha2::{Digest, Sha256};
use uuid::Uuid;
use xplan_cache::{CachedClientKey, LocalCache};
use xplan_db::PgPool;

pub struct AuthService {
    pool: PgPool,
    cache: LocalCache,
}

impl AuthService {
    pub fn new(pool: PgPool, cache: LocalCache) -> Self {
        Self { pool, cache }
    }

    pub async fn authenticate(&self, raw_key: &str) -> Result<CachedClientKey, AuthError> {
        let key_hash = hash_key(raw_key);

        // Check local cache first
        if let Some(cached) = self.cache.client_keys.get(&key_hash).await {
            if !cached.is_enabled {
                return Err(AuthError::Disabled);
            }
            return Ok(cached);
        }

        // Cache miss — query DB
        let row = sqlx::query_as::<_, ClientKeyRow>(
            "SELECT id, name, access_all_models, is_enabled, rate_limit_rpm FROM client_keys WHERE key_hash = $1"
        )
        .bind(&key_hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AuthError::Internal(e.to_string()))?;

        match row {
            Some(row) => {
                let cached = CachedClientKey {
                    id: row.id,
                    name: row.name,
                    access_all_models: row.access_all_models,
                    is_enabled: row.is_enabled,
                    rate_limit_rpm: row.rate_limit_rpm,
                };

                // Store in cache
                self.cache.client_keys.insert(key_hash, cached.clone()).await;

                if !cached.is_enabled {
                    return Err(AuthError::Disabled);
                }
                Ok(cached)
            }
            None => Err(AuthError::InvalidKey),
        }
    }

    pub async fn check_model_access(&self, client_key_id: Uuid, model_name: &str, access_all: bool) -> Result<(), AuthError> {
        if access_all {
            return Ok(());
        }

        let has_access = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(
                SELECT 1 FROM client_model_access cma
                JOIN models m ON m.id = cma.model_id
                WHERE cma.client_key_id = $1 AND m.name = $2 AND cma.is_enabled = true
            )"
        )
        .bind(client_key_id)
        .bind(model_name)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AuthError::Internal(e.to_string()))?;

        if has_access {
            Ok(())
        } else {
            Err(AuthError::ModelNotAllowed)
        }
    }
}

pub fn hash_key(raw_key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw_key.as_bytes());
    hex::encode(hasher.finalize())
}

#[derive(Debug, sqlx::FromRow)]
struct ClientKeyRow {
    id: Uuid,
    name: String,
    access_all_models: bool,
    is_enabled: bool,
    rate_limit_rpm: Option<i32>,
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("Invalid API key")]
    InvalidKey,
    #[error("API key disabled")]
    Disabled,
    #[error("Model not allowed for this key")]
    ModelNotAllowed,
    #[error("Internal error: {0}")]
    Internal(String),
}
```

- [ ] **Step 2: Implement router engine**

`crates/xplan-core/src/router.rs`:
```rust
use uuid::Uuid;
use xplan_cache::{LocalCache, QuotaTracker, RouteCandidate};
use xplan_db::PgPool;

pub struct RouterEngine {
    pool: PgPool,
    cache: LocalCache,
    quota_tracker: QuotaTracker,
}

impl RouterEngine {
    pub fn new(pool: PgPool, cache: LocalCache, quota_tracker: QuotaTracker) -> Self {
        Self { pool, cache, quota_tracker }
    }

    pub async fn select_upstream(&self, model_name: &str) -> Result<SelectedUpstream, RouterError> {
        let candidates = self.get_candidates(model_name).await?;

        if candidates.is_empty() {
            return Err(RouterError::NoRouteFound(model_name.to_string()));
        }

        // Group by priority
        let mut groups: std::collections::BTreeMap<i32, Vec<&RouteCandidate>> = std::collections::BTreeMap::new();
        for c in &candidates {
            groups.entry(c.priority).or_default().push(c);
        }

        // Try each priority group
        for (_priority, group) in &groups {
            // Filter by quota availability
            let mut available = Vec::new();
            for candidate in group {
                if self.check_quota(candidate.upstream_key_id).await? {
                    available.push(*candidate);
                }
            }

            if available.is_empty() {
                continue;
            }

            // Weighted random selection
            let selected = weighted_select(&available);
            return Ok(SelectedUpstream {
                upstream_key_id: selected.upstream_key_id,
                provider_model_id: selected.provider_model_id,
                provider_id: selected.provider_id,
                upstream_model_name: selected.upstream_model_name.clone(),
                base_url: selected.base_url.clone(),
                api_format: selected.api_format.clone(),
            });
        }

        Err(RouterError::AllUpstreamsExhausted(model_name.to_string()))
    }

    async fn get_candidates(&self, model_name: &str) -> Result<Vec<RouteCandidate>, RouterError> {
        // Check local cache
        if let Some(cached) = self.cache.route_table.get(model_name).await {
            return Ok(cached);
        }

        // Query DB
        let rows = sqlx::query_as::<_, RouteCandidateRow>(
            r#"
            SELECT
                kma.id as key_model_access_id,
                kma.upstream_key_id,
                kma.provider_model_id,
                p.id as provider_id,
                pm.upstream_model_name,
                p.base_url,
                p.api_format::text as api_format,
                kma.priority,
                kma.weight
            FROM models m
            JOIN provider_models pm ON pm.model_id = m.id
            JOIN key_model_access kma ON kma.provider_model_id = pm.id
            JOIN upstream_keys uk ON uk.id = kma.upstream_key_id
            JOIN providers p ON p.id = uk.provider_id
            WHERE m.name = $1
              AND m.is_enabled = true
              AND pm.provider_id = p.id
              AND kma.is_enabled = true
              AND uk.is_enabled = true
              AND uk.status != 'disabled'
              AND p.is_enabled = true
            ORDER BY kma.priority ASC, kma.weight DESC
            "#
        )
        .bind(model_name)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RouterError::Internal(e.to_string()))?;

        let candidates: Vec<RouteCandidate> = rows.into_iter().map(|r| RouteCandidate {
            key_model_access_id: r.key_model_access_id,
            upstream_key_id: r.upstream_key_id,
            provider_model_id: r.provider_model_id,
            provider_id: r.provider_id,
            upstream_model_name: r.upstream_model_name,
            base_url: r.base_url,
            api_format: r.api_format,
            priority: r.priority,
            weight: r.weight,
        }).collect();

        // Cache it
        self.cache.route_table.insert(model_name.to_string(), candidates.clone()).await;
        Ok(candidates)
    }

    async fn check_quota(&self, upstream_key_id: Uuid) -> Result<bool, RouterError> {
        let quotas = sqlx::query_as::<_, QuotaRow>(
            "SELECT id, limit_value, window_seconds FROM key_quotas WHERE upstream_key_id = $1"
        )
        .bind(upstream_key_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RouterError::Internal(e.to_string()))?;

        for quota in quotas {
            let result = self.quota_tracker.check_and_increment(
                upstream_key_id,
                quota.id,
                quota.limit_value,
                quota.window_seconds,
            ).await.map_err(|e| RouterError::Internal(e.to_string()))?;

            if !result.is_allowed() {
                return Ok(false);
            }
        }

        Ok(true)
    }
}

fn weighted_select(candidates: &[&RouteCandidate]) -> &RouteCandidate {
    use rand::Rng;
    let total_weight: i32 = candidates.iter().map(|c| c.weight).sum();
    if total_weight == 0 {
        return candidates[0];
    }
    let mut rng = rand::rng();
    let mut target = rng.random_range(0..total_weight);
    for candidate in candidates {
        target -= candidate.weight;
        if target < 0 {
            return candidate;
        }
    }
    candidates.last().unwrap()
}

#[derive(Debug, Clone)]
pub struct SelectedUpstream {
    pub upstream_key_id: Uuid,
    pub provider_model_id: Uuid,
    pub provider_id: Uuid,
    pub upstream_model_name: String,
    pub base_url: String,
    pub api_format: String,
}

#[derive(Debug, sqlx::FromRow)]
struct RouteCandidateRow {
    key_model_access_id: Uuid,
    upstream_key_id: Uuid,
    provider_model_id: Uuid,
    provider_id: Uuid,
    upstream_model_name: String,
    base_url: String,
    api_format: String,
    priority: i32,
    weight: i32,
}

#[derive(Debug, sqlx::FromRow)]
struct QuotaRow {
    id: Uuid,
    limit_value: i32,
    window_seconds: i32,
}

#[derive(Debug, thiserror::Error)]
pub enum RouterError {
    #[error("No route found for model: {0}")]
    NoRouteFound(String),
    #[error("All upstreams exhausted for model: {0}")]
    AllUpstreamsExhausted(String),
    #[error("Internal error: {0}")]
    Internal(String),
}
```

- [ ] **Step 3: Implement billing engine**

`crates/xplan-core/src/billing.rs`:
```rust
use uuid::Uuid;
use xplan_cache::{LocalCache, PriceEntry};
use xplan_db::PgPool;
use xplan_provider::TokenUsage;

pub struct BillingEngine {
    pool: PgPool,
    cache: LocalCache,
}

impl BillingEngine {
    pub fn new(pool: PgPool, cache: LocalCache) -> Self {
        Self { pool, cache }
    }

    pub async fn calculate_cost(&self, provider_model_id: Uuid, usage: &TokenUsage) -> anyhow::Result<i32> {
        let prices = self.get_prices(provider_model_id).await?;

        let cost = (usage.input_tokens as i64 * prices.input_price_per_mtok as i64) / 1_000_000
            + (usage.output_tokens as i64 * prices.output_price_per_mtok as i64) / 1_000_000
            + (usage.cache_read_tokens as i64 * prices.cache_read_price_per_mtok as i64) / 1_000_000
            + (usage.cache_write_tokens as i64 * prices.cache_write_price_per_mtok as i64) / 1_000_000;

        Ok(cost as i32)
    }

    async fn get_prices(&self, provider_model_id: Uuid) -> anyhow::Result<PriceEntry> {
        if let Some(cached) = self.cache.price_table.get(&provider_model_id).await {
            return Ok(cached);
        }

        let row = sqlx::query_as::<_, PriceRow>(
            "SELECT input_price_per_mtok, output_price_per_mtok, cache_read_price_per_mtok, cache_write_price_per_mtok FROM provider_models WHERE id = $1"
        )
        .bind(provider_model_id)
        .fetch_one(&self.pool)
        .await?;

        let entry = PriceEntry {
            input_price_per_mtok: row.input_price_per_mtok,
            output_price_per_mtok: row.output_price_per_mtok,
            cache_read_price_per_mtok: row.cache_read_price_per_mtok,
            cache_write_price_per_mtok: row.cache_write_price_per_mtok,
        };

        self.cache.price_table.insert(provider_model_id, entry.clone()).await;
        Ok(entry)
    }
}

#[derive(Debug, sqlx::FromRow)]
struct PriceRow {
    input_price_per_mtok: i32,
    output_price_per_mtok: i32,
    cache_read_price_per_mtok: i32,
    cache_write_price_per_mtok: i32,
}
```

- [ ] **Step 4: Implement quality monitor**

`crates/xplan-core/src/quality.rs`:
```rust
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use dashmap::DashMap;
use uuid::Uuid;

pub struct QualityMonitor {
    windows: Arc<DashMap<Uuid, SlidingWindow>>,
}

impl QualityMonitor {
    pub fn new() -> Self {
        Self {
            windows: Arc::new(DashMap::new()),
        }
    }

    pub fn record(&self, provider_model_id: Uuid, success: bool, latency_ms: u32) {
        self.windows
            .entry(provider_model_id)
            .or_insert_with(|| SlidingWindow::new(Duration::from_secs(3600)))
            .record(success, latency_ms);
    }

    pub fn quality_factor(&self, provider_model_id: Uuid) -> f64 {
        self.windows
            .get(&provider_model_id)
            .map(|w| w.quality_factor())
            .unwrap_or(1.0) // Default to neutral if no data
    }

    pub fn stats(&self, provider_model_id: Uuid) -> Option<QualitySnapshot> {
        self.windows.get(&provider_model_id).map(|w| w.snapshot())
    }
}

struct SlidingWindow {
    buckets: VecDeque<TimeBucket>,
    bucket_duration: Duration,
    window_size: Duration,
    last_bucket_time: Instant,
}

#[derive(Default, Clone)]
struct TimeBucket {
    success: u32,
    failure: u32,
    total_latency_ms: u64,
    count: u32,
}

impl SlidingWindow {
    fn new(window_size: Duration) -> Self {
        Self {
            buckets: VecDeque::new(),
            bucket_duration: Duration::from_secs(60),
            window_size,
            last_bucket_time: Instant::now(),
        }
    }

    fn record(&mut self, success: bool, latency_ms: u32) {
        self.advance_time();
        let bucket = self.buckets.back_mut().unwrap();
        if success {
            bucket.success += 1;
        } else {
            bucket.failure += 1;
        }
        bucket.total_latency_ms += latency_ms as u64;
        bucket.count += 1;
    }

    fn advance_time(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_bucket_time);

        if elapsed >= self.bucket_duration || self.buckets.is_empty() {
            self.buckets.push_back(TimeBucket::default());
            self.last_bucket_time = now;

            let max_buckets = (self.window_size.as_secs() / self.bucket_duration.as_secs()) as usize;
            while self.buckets.len() > max_buckets {
                self.buckets.pop_front();
            }
        }
    }

    fn quality_factor(&self) -> f64 {
        let (total_success, total_failure, total_latency, total_count) = self.buckets.iter().fold(
            (0u64, 0u64, 0u64, 0u64),
            |(s, f, l, c), b| (s + b.success as u64, f + b.failure as u64, l + b.total_latency_ms, c + b.count as u64),
        );

        if total_count == 0 {
            return 1.0;
        }

        let success_rate = total_success as f64 / (total_success + total_failure) as f64;
        let avg_latency_ms = total_latency as f64 / total_count as f64;

        // Normalize: fast responses (< 1s) get factor ~1.0, slow (> 10s) get penalized
        let latency_factor = (1000.0 / avg_latency_ms.max(100.0)).min(1.0);

        success_rate * latency_factor
    }

    fn snapshot(&self) -> QualitySnapshot {
        let (total_success, total_failure, total_latency, total_count) = self.buckets.iter().fold(
            (0u32, 0u32, 0u64, 0u32),
            |(s, f, l, c), b| (s + b.success, f + b.failure, l + b.total_latency_ms, c + b.count),
        );

        QualitySnapshot {
            total_requests: total_count,
            success_count: total_success,
            error_count: total_failure,
            avg_latency_ms: if total_count > 0 { (total_latency / total_count as u64) as u32 } else { 0 },
        }
    }
}

#[derive(Debug, Clone)]
pub struct QualitySnapshot {
    pub total_requests: u32,
    pub success_count: u32,
    pub error_count: u32,
    pub avg_latency_ms: u32,
}
```

Add `dashmap = "6"` to `crates/xplan-core/Cargo.toml` dependencies.

- [ ] **Step 5: Wire up lib.rs**

`crates/xplan-core/src/lib.rs`:
```rust
pub mod auth;
pub mod router;
pub mod billing;
pub mod quality;

pub use auth::{AuthService, AuthError};
pub use router::{RouterEngine, SelectedUpstream, RouterError};
pub use billing::BillingEngine;
pub use quality::QualityMonitor;
```

- [ ] **Step 6: Verify compilation**

Run: `cargo build`
Expected: successful compilation

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: add core domain logic - auth, router, billing, quality monitor"
```

---

## Task 6: Database Repository Layer

**Files:**
- Create: `crates/xplan-db/src/repo_provider.rs`
- Create: `crates/xplan-db/src/repo_upstream_key.rs`
- Create: `crates/xplan-db/src/repo_model.rs`
- Create: `crates/xplan-db/src/repo_client_key.rs`
- Create: `crates/xplan-db/src/repo_usage.rs`
- Create: `crates/xplan-db/src/repo_quality.rs`
- Modify: `crates/xplan-db/src/lib.rs`

- [ ] **Step 1: Provider repository**

`crates/xplan-db/src/repo_provider.rs`:
```rust
use crate::models::{ApiFormat, Provider};
use sqlx::PgPool;
use uuid::Uuid;

pub async fn list_providers(pool: &PgPool) -> anyhow::Result<Vec<Provider>> {
    let rows = sqlx::query_as::<_, Provider>("SELECT * FROM providers ORDER BY name")
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

pub async fn get_provider(pool: &PgPool, id: Uuid) -> anyhow::Result<Option<Provider>> {
    let row = sqlx::query_as::<_, Provider>("SELECT * FROM providers WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn create_provider(
    pool: &PgPool,
    name: &str,
    base_url: &str,
    api_format: ApiFormat,
) -> anyhow::Result<Provider> {
    let row = sqlx::query_as::<_, Provider>(
        "INSERT INTO providers (name, base_url, api_format) VALUES ($1, $2, $3) RETURNING *"
    )
    .bind(name)
    .bind(base_url)
    .bind(api_format)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn update_provider(
    pool: &PgPool,
    id: Uuid,
    name: &str,
    base_url: &str,
    api_format: ApiFormat,
    is_enabled: bool,
) -> anyhow::Result<Option<Provider>> {
    let row = sqlx::query_as::<_, Provider>(
        "UPDATE providers SET name=$2, base_url=$3, api_format=$4, is_enabled=$5 WHERE id=$1 RETURNING *"
    )
    .bind(id)
    .bind(name)
    .bind(base_url)
    .bind(api_format)
    .bind(is_enabled)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn delete_provider(pool: &PgPool, id: Uuid) -> anyhow::Result<bool> {
    let result = sqlx::query("DELETE FROM providers WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}
```

- [ ] **Step 2: Upstream key repository**

`crates/xplan-db/src/repo_upstream_key.rs`:
```rust
use crate::models::{KeyStatus, UpstreamKey};
use sqlx::PgPool;
use uuid::Uuid;

pub async fn list_upstream_keys(pool: &PgPool, provider_id: Option<Uuid>) -> anyhow::Result<Vec<UpstreamKey>> {
    if let Some(pid) = provider_id {
        let rows = sqlx::query_as::<_, UpstreamKey>(
            "SELECT * FROM upstream_keys WHERE provider_id = $1 ORDER BY alias"
        )
        .bind(pid)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    } else {
        let rows = sqlx::query_as::<_, UpstreamKey>("SELECT * FROM upstream_keys ORDER BY alias")
            .fetch_all(pool)
            .await?;
        Ok(rows)
    }
}

pub async fn get_upstream_key(pool: &PgPool, id: Uuid) -> anyhow::Result<Option<UpstreamKey>> {
    let row = sqlx::query_as::<_, UpstreamKey>("SELECT * FROM upstream_keys WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn create_upstream_key(
    pool: &PgPool,
    provider_id: Uuid,
    alias: &str,
    api_key_encrypted: &[u8],
) -> anyhow::Result<UpstreamKey> {
    let row = sqlx::query_as::<_, UpstreamKey>(
        "INSERT INTO upstream_keys (provider_id, alias, api_key_encrypted) VALUES ($1, $2, $3) RETURNING *"
    )
    .bind(provider_id)
    .bind(alias)
    .bind(api_key_encrypted)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn update_upstream_key_status(pool: &PgPool, id: Uuid, status: KeyStatus) -> anyhow::Result<()> {
    sqlx::query("UPDATE upstream_keys SET status = $2 WHERE id = $1")
        .bind(id)
        .bind(status)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_upstream_key(pool: &PgPool, id: Uuid) -> anyhow::Result<bool> {
    let result = sqlx::query("DELETE FROM upstream_keys WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}
```

- [ ] **Step 3: Model repository**

`crates/xplan-db/src/repo_model.rs`:
```rust
use crate::models::{Model, ProviderModel, KeyModelAccess};
use sqlx::PgPool;
use uuid::Uuid;

pub async fn list_models(pool: &PgPool) -> anyhow::Result<Vec<Model>> {
    let rows = sqlx::query_as::<_, Model>("SELECT * FROM models ORDER BY name")
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

pub async fn create_model(pool: &PgPool, name: &str) -> anyhow::Result<Model> {
    let row = sqlx::query_as::<_, Model>(
        "INSERT INTO models (name) VALUES ($1) RETURNING *"
    )
    .bind(name)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn delete_model(pool: &PgPool, id: Uuid) -> anyhow::Result<bool> {
    let result = sqlx::query("DELETE FROM models WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn list_provider_models(pool: &PgPool, model_id: Option<Uuid>) -> anyhow::Result<Vec<ProviderModel>> {
    if let Some(mid) = model_id {
        let rows = sqlx::query_as::<_, ProviderModel>(
            "SELECT * FROM provider_models WHERE model_id = $1"
        )
        .bind(mid)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    } else {
        let rows = sqlx::query_as::<_, ProviderModel>("SELECT * FROM provider_models")
            .fetch_all(pool)
            .await?;
        Ok(rows)
    }
}

pub async fn create_provider_model(
    pool: &PgPool,
    provider_id: Uuid,
    model_id: Uuid,
    upstream_model_name: &str,
    input_price: i32,
    output_price: i32,
    cache_read_price: i32,
    cache_write_price: i32,
) -> anyhow::Result<ProviderModel> {
    let row = sqlx::query_as::<_, ProviderModel>(
        r#"INSERT INTO provider_models
           (provider_id, model_id, upstream_model_name, input_price_per_mtok, output_price_per_mtok, cache_read_price_per_mtok, cache_write_price_per_mtok)
           VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING *"#
    )
    .bind(provider_id)
    .bind(model_id)
    .bind(upstream_model_name)
    .bind(input_price)
    .bind(output_price)
    .bind(cache_read_price)
    .bind(cache_write_price)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn create_key_model_access(
    pool: &PgPool,
    upstream_key_id: Uuid,
    provider_model_id: Uuid,
    priority: i32,
    weight: i32,
) -> anyhow::Result<KeyModelAccess> {
    let row = sqlx::query_as::<_, KeyModelAccess>(
        "INSERT INTO key_model_access (upstream_key_id, provider_model_id, priority, weight) VALUES ($1, $2, $3, $4) RETURNING *"
    )
    .bind(upstream_key_id)
    .bind(provider_model_id)
    .bind(priority)
    .bind(weight)
    .fetch_one(pool)
    .await?;
    Ok(row)
}
```

- [ ] **Step 4: Client key repository**

`crates/xplan-db/src/repo_client_key.rs`:
```rust
use crate::models::ClientKey;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn list_client_keys(pool: &PgPool) -> anyhow::Result<Vec<ClientKey>> {
    let rows = sqlx::query_as::<_, ClientKey>("SELECT * FROM client_keys ORDER BY name")
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

pub async fn create_client_key(
    pool: &PgPool,
    name: &str,
    key_hash: &str,
    key_prefix: &str,
    access_all_models: bool,
    rate_limit_rpm: Option<i32>,
) -> anyhow::Result<ClientKey> {
    let row = sqlx::query_as::<_, ClientKey>(
        r#"INSERT INTO client_keys (name, key_hash, key_prefix, access_all_models, rate_limit_rpm)
           VALUES ($1, $2, $3, $4, $5) RETURNING *"#
    )
    .bind(name)
    .bind(key_hash)
    .bind(key_prefix)
    .bind(access_all_models)
    .bind(rate_limit_rpm)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn update_client_key_enabled(pool: &PgPool, id: Uuid, is_enabled: bool) -> anyhow::Result<()> {
    sqlx::query("UPDATE client_keys SET is_enabled = $2 WHERE id = $1")
        .bind(id)
        .bind(is_enabled)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_client_key(pool: &PgPool, id: Uuid) -> anyhow::Result<bool> {
    let result = sqlx::query("DELETE FROM client_keys WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}
```

- [ ] **Step 5: Usage log repository**

`crates/xplan-db/src/repo_usage.rs`:
```rust
use crate::models::UsageLog;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

pub struct UsageLogInsert {
    pub client_key_id: Uuid,
    pub upstream_key_id: Uuid,
    pub provider_model_id: Uuid,
    pub model_name: String,
    pub provider_name: String,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub cache_read_tokens: i32,
    pub cache_write_tokens: i32,
    pub cost_cents: i32,
    pub latency_ms: i32,
    pub ttft_ms: Option<i32>,
    pub status_code: i32,
    pub is_success: bool,
    pub error_type: Option<String>,
}

pub async fn insert_usage_log(pool: &PgPool, log: &UsageLogInsert) -> anyhow::Result<()> {
    sqlx::query(
        r#"INSERT INTO usage_logs
           (client_key_id, upstream_key_id, provider_model_id, model_name, provider_name,
            input_tokens, output_tokens, cache_read_tokens, cache_write_tokens,
            cost_cents, latency_ms, ttft_ms, status_code, is_success, error_type)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)"#
    )
    .bind(log.client_key_id)
    .bind(log.upstream_key_id)
    .bind(log.provider_model_id)
    .bind(&log.model_name)
    .bind(&log.provider_name)
    .bind(log.input_tokens)
    .bind(log.output_tokens)
    .bind(log.cache_read_tokens)
    .bind(log.cache_write_tokens)
    .bind(log.cost_cents)
    .bind(log.latency_ms)
    .bind(log.ttft_ms)
    .bind(log.status_code)
    .bind(log.is_success)
    .bind(&log.error_type)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn query_usage(
    pool: &PgPool,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    model_name: Option<&str>,
    provider_name: Option<&str>,
    limit: i64,
    offset: i64,
) -> anyhow::Result<Vec<UsageLog>> {
    let rows = sqlx::query_as::<_, UsageLog>(
        r#"SELECT * FROM usage_logs
           WHERE created_at >= $1 AND created_at < $2
             AND ($3::text IS NULL OR model_name = $3)
             AND ($4::text IS NULL OR provider_name = $4)
           ORDER BY created_at DESC
           LIMIT $5 OFFSET $6"#
    )
    .bind(from)
    .bind(to)
    .bind(model_name)
    .bind(provider_name)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct UsageSummary {
    pub model_name: String,
    pub provider_name: String,
    pub total_requests: i64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub total_cost_cents: i64,
    pub avg_latency_ms: f64,
}

pub async fn usage_summary(
    pool: &PgPool,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> anyhow::Result<Vec<UsageSummary>> {
    let rows = sqlx::query_as::<_, UsageSummary>(
        r#"SELECT
             model_name,
             provider_name,
             COUNT(*) as total_requests,
             SUM(input_tokens)::bigint as total_input_tokens,
             SUM(output_tokens)::bigint as total_output_tokens,
             SUM(cost_cents)::bigint as total_cost_cents,
             AVG(latency_ms)::float8 as avg_latency_ms
           FROM usage_logs
           WHERE created_at >= $1 AND created_at < $2
           GROUP BY model_name, provider_name
           ORDER BY total_requests DESC"#
    )
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
```

- [ ] **Step 6: Quality stats repository**

`crates/xplan-db/src/repo_quality.rs`:
```rust
use crate::models::{PeriodType, QualityStat};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

pub async fn upsert_quality_stat(
    pool: &PgPool,
    provider_model_id: Uuid,
    period_start: DateTime<Utc>,
    period_type: PeriodType,
    total_requests: i32,
    success_count: i32,
    error_count: i32,
    avg_latency_ms: i32,
    p95_latency_ms: i32,
    avg_ttft_ms: Option<i32>,
    total_tokens: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"INSERT INTO quality_stats
           (provider_model_id, period_start, period_type, total_requests, success_count, error_count,
            avg_latency_ms, p95_latency_ms, avg_ttft_ms, total_tokens)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
           ON CONFLICT (provider_model_id, period_start, period_type)
           DO UPDATE SET
             total_requests = EXCLUDED.total_requests,
             success_count = EXCLUDED.success_count,
             error_count = EXCLUDED.error_count,
             avg_latency_ms = EXCLUDED.avg_latency_ms,
             p95_latency_ms = EXCLUDED.p95_latency_ms,
             avg_ttft_ms = EXCLUDED.avg_ttft_ms,
             total_tokens = EXCLUDED.total_tokens"#
    )
    .bind(provider_model_id)
    .bind(period_start)
    .bind(period_type)
    .bind(total_requests)
    .bind(success_count)
    .bind(error_count)
    .bind(avg_latency_ms)
    .bind(p95_latency_ms)
    .bind(avg_ttft_ms)
    .bind(total_tokens)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn query_quality_stats(
    pool: &PgPool,
    provider_model_id: Option<Uuid>,
    period_type: PeriodType,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> anyhow::Result<Vec<QualityStat>> {
    let rows = sqlx::query_as::<_, QualityStat>(
        r#"SELECT * FROM quality_stats
           WHERE ($1::uuid IS NULL OR provider_model_id = $1)
             AND period_type = $2
             AND period_start >= $3 AND period_start < $4
           ORDER BY period_start DESC"#
    )
    .bind(provider_model_id)
    .bind(period_type)
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
```

- [ ] **Step 7: Update xplan-db lib.rs**

`crates/xplan-db/src/lib.rs`:
```rust
pub mod models;
pub mod repo_provider;
pub mod repo_upstream_key;
pub mod repo_model;
pub mod repo_client_key;
pub mod repo_usage;
pub mod repo_quality;

pub use sqlx::PgPool;

pub async fn create_pool(database_url: &str, max_connections: u32) -> anyhow::Result<PgPool> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(max_connections)
        .connect(database_url)
        .await?;
    Ok(pool)
}
```

- [ ] **Step 8: Verify compilation**

Run: `cargo build`
Expected: successful compilation

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "feat: add database repository layer with CRUD for all entities"
```

---

## Task 7: Server Setup + AppState + Auth Middleware

**Files:**
- Create: `crates/xplan-server/src/state.rs`
- Create: `crates/xplan-server/src/middleware/mod.rs`
- Create: `crates/xplan-server/src/middleware/auth.rs`
- Modify: `crates/xplan-server/src/main.rs`

- [ ] **Step 1: Define AppState**

`crates/xplan-server/src/state.rs`:
```rust
use std::sync::Arc;
use xplan_cache::{LocalCache, QuotaTracker, InvalidationPublisher};
use xplan_core::{AuthService, BillingEngine, QualityMonitor, RouterEngine};
use xplan_db::PgPool;
use xplan_provider::openai::OpenAiAdapter;
use xplan_provider::anthropic::AnthropicAdapter;
use xplan_provider::bedrock::BedrockConverseAdapter;

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
    pub bedrock_adapter: BedrockConverseAdapter,
    pub encryption_key: Vec<u8>,
    pub admin_token: String,
}

impl AppState {
    pub fn auth(&self) -> &AuthService { &self.inner.auth }
    pub fn router(&self) -> &RouterEngine { &self.inner.router }
    pub fn billing(&self) -> &BillingEngine { &self.inner.billing }
    pub fn quality(&self) -> &QualityMonitor { &self.inner.quality }
    pub fn pool(&self) -> &PgPool { &self.inner.pool }
    pub fn cache(&self) -> &LocalCache { &self.inner.cache }
    pub fn publisher(&self) -> &InvalidationPublisher { &self.inner.invalidation_publisher }
    pub fn encryption_key(&self) -> &[u8] { &self.inner.encryption_key }
    pub fn admin_token(&self) -> &str { &self.inner.admin_token }
}
```

- [ ] **Step 2: Create auth middleware**

`crates/xplan-server/src/middleware/mod.rs`:
```rust
pub mod auth;
```

`crates/xplan-server/src/middleware/auth.rs`:
```rust
use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Extension;
use xplan_cache::CachedClientKey;

use crate::state::AppState;

pub async fn proxy_auth(
    state: axum::extract::State<AppState>,
    mut req: Request,
    next: Next,
) -> Response {
    let api_key = extract_api_key(req.headers());

    let Some(api_key) = api_key else {
        return (StatusCode::UNAUTHORIZED, r#"{"error":"Missing API key"}"#).into_response();
    };

    match state.auth().authenticate(&api_key).await {
        Ok(client) => {
            req.extensions_mut().insert(client);
            next.run(req).await
        }
        Err(e) => {
            let msg = format!(r#"{{"error":"{}"}}"#, e);
            (StatusCode::UNAUTHORIZED, msg).into_response()
        }
    }
}

pub async fn admin_auth(
    state: axum::extract::State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    let token = extract_api_key(req.headers());

    let Some(token) = token else {
        return (StatusCode::UNAUTHORIZED, r#"{"error":"Missing admin token"}"#).into_response();
    };

    if token != state.admin_token() {
        return (StatusCode::UNAUTHORIZED, r#"{"error":"Invalid admin token"}"#).into_response();
    }

    next.run(req).await
}

fn extract_api_key(headers: &axum::http::HeaderMap) -> Option<String> {
    // Try Authorization: Bearer first
    if let Some(auth) = headers.get("authorization") {
        if let Ok(val) = auth.to_str() {
            if let Some(key) = val.strip_prefix("Bearer ") {
                return Some(key.to_string());
            }
        }
    }
    // Then x-api-key
    if let Some(key) = headers.get("x-api-key") {
        if let Ok(val) = key.to_str() {
            return Some(val.to_string());
        }
    }
    None
}
```

- [ ] **Step 3: Update main.rs with full initialization**

`crates/xplan-server/src/main.rs`:
```rust
mod config;
mod state;
mod middleware;

use config::AppConfig;
use state::{AppState, AppStateInner};
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use xplan_cache::{
    create_redis_pool, spawn_invalidation_listener, InvalidationPublisher, LocalCache, QuotaTracker,
};
use xplan_core::{AuthService, BillingEngine, QualityMonitor, RouterEngine};
use xplan_provider::{anthropic::AnthropicAdapter, openai::OpenAiAdapter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "xplan=debug,tower_http=debug".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = AppConfig::load()?;
    tracing::info!("Starting xplan on {}:{}", config.server.host, config.server.port);

    // Database
    let pool = xplan_db::create_pool(&config.database.url, config.database.max_connections).await?;
    tracing::info!("Database connected");

    // Redis
    let redis_pool = create_redis_pool(&config.redis.url)?;
    tracing::info!("Redis pool created");

    // Cache
    let local_cache = LocalCache::new();
    let quota_tracker = QuotaTracker::new(redis_pool.clone());
    let invalidation_publisher = InvalidationPublisher::new(redis_pool.clone());

    // Spawn cache invalidation listener
    let (inv_tx, _inv_rx) = tokio::sync::broadcast::channel(64);
    spawn_invalidation_listener(config.redis.url.clone(), inv_tx.clone());

    // Spawn local cache invalidation handler
    let cache_for_inv = local_cache.clone();
    let mut inv_rx = inv_tx.subscribe();
    tokio::spawn(async move {
        while let Ok(event) = inv_rx.recv().await {
            match event {
                xplan_cache::InvalidationEvent::Routes => cache_for_inv.invalidate_routes(),
                xplan_cache::InvalidationEvent::ClientKey(hash) => cache_for_inv.invalidate_client_key(&hash),
                xplan_cache::InvalidationEvent::Prices => cache_for_inv.price_table.invalidate_all(),
                xplan_cache::InvalidationEvent::All => cache_for_inv.invalidate_all(),
            }
        }
    });

    // Core services
    let auth = AuthService::new(pool.clone(), local_cache.clone());
    let router_engine = RouterEngine::new(pool.clone(), local_cache.clone(), quota_tracker);
    let billing = BillingEngine::new(pool.clone(), local_cache.clone());
    let quality = QualityMonitor::new();

    // Encryption key
    let encryption_key = hex::decode(&config.encryption.key_hex)?;

    // App state
    let state = AppState {
        inner: Arc::new(AppStateInner {
            pool,
            auth,
            router: router_engine,
            billing,
            quality,
            cache: local_cache,
            invalidation_publisher,
            openai_adapter: OpenAiAdapter::new(),
            anthropic_adapter: AnthropicAdapter::new(),
            bedrock_adapter: BedrockConverseAdapter::new(),
            encryption_key,
            admin_token: config.admin.token.clone(),
        }),
    };

    // Router
    let app = axum::Router::new()
        .route("/health", axum::routing::get(|| async { "ok" }))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("{}:{}", config.server.host, config.server.port)).await?;
    tracing::info!("Listening on {}", listener.local_addr()?);
    axum::serve(listener, app).await?;
    Ok(())
}
```

- [ ] **Step 4: Verify compilation**

Run: `cargo build`
Expected: successful compilation

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: add AppState, auth middleware, and full server initialization"
```

---

## Task 8: Proxy Handlers (chat/completions, messages, models)

**Files:**
- Create: `crates/xplan-server/src/proxy/mod.rs`
- Create: `crates/xplan-server/src/proxy/chat.rs`
- Create: `crates/xplan-server/src/proxy/messages.rs`
- Create: `crates/xplan-server/src/proxy/models.rs`
- Modify: `crates/xplan-server/src/main.rs`

- [ ] **Step 1: Create proxy router module**

`crates/xplan-server/src/proxy/mod.rs`:
```rust
pub mod chat;
pub mod messages;
pub mod models;

use axum::{middleware, routing::{get, post}, Router};
use crate::state::AppState;
use crate::middleware::auth::proxy_auth;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/chat/completions", post(chat::handle_chat_completion))
        .route("/v1/messages", post(messages::handle_messages))
        .route("/v1/models", get(models::handle_list_models))
        .layer(middleware::from_fn_with_state(AppState::default_placeholder(), proxy_auth))
}
```

Actually, axum's middleware layer needs state passed differently. Let me use the simpler pattern:

```rust
pub mod chat;
pub mod messages;
pub mod models;

use axum::{middleware, routing::{get, post}, Router};
use crate::state::AppState;
use crate::middleware::auth::proxy_auth;

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/v1/chat/completions", post(chat::handle_chat_completion))
        .route("/v1/messages", post(messages::handle_messages))
        .route("/v1/models", get(models::handle_list_models))
        .route_layer(middleware::from_fn_with_state(state, proxy_auth))
}
```

- [ ] **Step 2: Implement chat completions handler**

`crates/xplan-server/src/proxy/chat.rs`:
```rust
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response, Sse},
    Extension, Json,
};
use std::time::Instant;
use uuid::Uuid;
use xplan_cache::CachedClientKey;
use xplan_core::RouterError;
use xplan_db::repo_usage::UsageLogInsert;
use xplan_provider::{ProviderAdapter, UpstreamRequest};

use crate::state::AppState;

pub async fn handle_chat_completion(
    State(state): State<AppState>,
    Extension(client): Extension<CachedClientKey>,
    Json(mut body): Json<serde_json::Value>,
) -> Response {
    let model_name = body["model"].as_str().unwrap_or("").to_string();
    let is_stream = body["stream"].as_bool().unwrap_or(false);

    // Check model access
    if let Err(e) = state.auth().check_model_access(client.id, &model_name, client.access_all_models).await {
        return (StatusCode::FORBIDDEN, Json(serde_json::json!({"error": e.to_string()}))).into_response();
    }

    // Route to upstream
    let selected = match state.router().select_upstream(&model_name).await {
        Ok(s) => s,
        Err(RouterError::NoRouteFound(_)) => {
            return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Model not found"}))).into_response();
        }
        Err(RouterError::AllUpstreamsExhausted(_)) => {
            return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "All upstreams exhausted"}))).into_response();
        }
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response();
        }
    };

    // Replace model name with upstream model name
    body["model"] = serde_json::Value::String(selected.upstream_model_name.clone());

    // Decrypt upstream key
    let upstream_key = match get_decrypted_key(&state, selected.upstream_key_id).await {
        Ok(k) => k,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response();
        }
    };

    let req: UpstreamRequest = match serde_json::from_value(body) {
        Ok(r) => r,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e.to_string()}))).into_response();
        }
    };

    let start = Instant::now();

    // Non-streaming path
    if !is_stream {
        let result = match selected.api_format.as_str() {
            "anthropic" => state.inner.anthropic_adapter.chat_completion(&selected.base_url, &upstream_key, req).await,
            "bedrock" => state.inner.bedrock_adapter.chat_completion(&selected.base_url, &upstream_key, req).await,
            _ => state.inner.openai_adapter.chat_completion(&selected.base_url, &upstream_key, req).await,
        };

        let latency_ms = start.elapsed().as_millis() as i32;

        match result {
            Ok(resp) => {
                let cost = state.billing().calculate_cost(selected.provider_model_id, &resp.usage).await.unwrap_or(0);

                // Record quality
                state.quality().record(selected.provider_model_id, true, latency_ms as u32);

                // Async usage log
                let pool = state.pool().clone();
                let log = UsageLogInsert {
                    client_key_id: client.id,
                    upstream_key_id: selected.upstream_key_id,
                    provider_model_id: selected.provider_model_id,
                    model_name: model_name.clone(),
                    provider_name: get_provider_name(&state, selected.provider_id).await,
                    input_tokens: resp.usage.input_tokens,
                    output_tokens: resp.usage.output_tokens,
                    cache_read_tokens: resp.usage.cache_read_tokens,
                    cache_write_tokens: resp.usage.cache_write_tokens,
                    cost_cents: cost,
                    latency_ms,
                    ttft_ms: None,
                    status_code: resp.status as i32,
                    is_success: true,
                    error_type: None,
                };
                tokio::spawn(async move {
                    let _ = xplan_db::repo_usage::insert_usage_log(&pool, &log).await;
                });

                Json(resp.body).into_response()
            }
            Err(e) => {
                state.quality().record(selected.provider_model_id, false, latency_ms as u32);
                let status = match &e {
                    xplan_provider::ProviderError::Http { status, .. } => StatusCode::from_u16(*status).unwrap_or(StatusCode::BAD_GATEWAY),
                    _ => StatusCode::BAD_GATEWAY,
                };
                (status, Json(serde_json::json!({"error": e.to_string()}))).into_response()
            }
        }
    } else {
        // Streaming path - forward SSE chunks directly
        let result = match selected.api_format.as_str() {
            "anthropic" => state.inner.anthropic_adapter.chat_completion_stream(&selected.base_url, &upstream_key, req).await,
            "bedrock" => state.inner.bedrock_adapter.chat_completion_stream(&selected.base_url, &upstream_key, req).await,
            _ => state.inner.openai_adapter.chat_completion_stream(&selected.base_url, &upstream_key, req).await,
        };

        match result {
            Ok(stream_resp) => {
                let state_clone = state.clone();
                let client_id = client.id;

                // Return the stream as SSE
                let body = axum::body::Body::from_stream(stream_resp.stream);
                let mut response = Response::new(body);
                response.headers_mut().insert("content-type", "text/event-stream".parse().unwrap());
                response.headers_mut().insert("cache-control", "no-cache".parse().unwrap());

                // Spawn task to record usage after stream completes
                tokio::spawn(async move {
                    if let Ok(usage) = stream_resp.usage.await {
                        let latency_ms = start.elapsed().as_millis() as i32;
                        let cost = state_clone.billing().calculate_cost(selected.provider_model_id, &usage).await.unwrap_or(0);
                        state_clone.quality().record(selected.provider_model_id, true, latency_ms as u32);

                        let log = UsageLogInsert {
                            client_key_id: client_id,
                            upstream_key_id: selected.upstream_key_id,
                            provider_model_id: selected.provider_model_id,
                            model_name,
                            provider_name: get_provider_name(&state_clone, selected.provider_id).await,
                            input_tokens: usage.input_tokens,
                            output_tokens: usage.output_tokens,
                            cache_read_tokens: usage.cache_read_tokens,
                            cache_write_tokens: usage.cache_write_tokens,
                            cost_cents: cost,
                            latency_ms,
                            ttft_ms: None,
                            status_code: 200,
                            is_success: true,
                            error_type: None,
                        };
                        let _ = xplan_db::repo_usage::insert_usage_log(state_clone.pool(), &log).await;
                    }
                });

                response
            }
            Err(e) => {
                (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"error": e.to_string()}))).into_response()
            }
        }
    }
}

async fn get_decrypted_key(state: &AppState, upstream_key_id: Uuid) -> anyhow::Result<String> {
    let row = sqlx::query_scalar::<_, Vec<u8>>(
        "SELECT api_key_encrypted FROM upstream_keys WHERE id = $1"
    )
    .bind(upstream_key_id)
    .fetch_one(state.pool())
    .await?;

    decrypt_key(&row, state.encryption_key())
}

fn decrypt_key(encrypted: &[u8], key: &[u8]) -> anyhow::Result<String> {
    use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
    use aes_gcm::aead::Aead;

    if encrypted.len() < 12 {
        anyhow::bail!("Invalid encrypted key: too short");
    }

    let cipher = Aes256Gcm::new_from_slice(key)?;
    let nonce = Nonce::from_slice(&encrypted[..12]);
    let ciphertext = &encrypted[12..];
    let plaintext = cipher.decrypt(nonce, ciphertext)
        .map_err(|e| anyhow::anyhow!("Decryption failed: {}", e))?;
    Ok(String::from_utf8(plaintext)?)
}

async fn get_provider_name(state: &AppState, provider_id: Uuid) -> String {
    sqlx::query_scalar::<_, String>("SELECT name FROM providers WHERE id = $1")
        .bind(provider_id)
        .fetch_one(state.pool())
        .await
        .unwrap_or_else(|_| "unknown".to_string())
}
```

- [ ] **Step 3: Implement Anthropic messages handler**

`crates/xplan-server/src/proxy/messages.rs`:
```rust
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Extension, Json,
};
use std::time::Instant;
use xplan_cache::CachedClientKey;
use xplan_db::repo_usage::UsageLogInsert;
use xplan_provider::{ProviderAdapter, UpstreamRequest, types::MessageContent, types::Message};

use crate::state::AppState;
use super::chat::{get_decrypted_key, get_provider_name};

pub async fn handle_messages(
    State(state): State<AppState>,
    Extension(client): Extension<CachedClientKey>,
    Json(body): Json<serde_json::Value>,
) -> Response {
    let model_name = body["model"].as_str().unwrap_or("").to_string();
    let is_stream = body["stream"].as_bool().unwrap_or(false);

    // Check model access
    if let Err(e) = state.auth().check_model_access(client.id, &model_name, client.access_all_models).await {
        return (StatusCode::FORBIDDEN, Json(serde_json::json!({"error": e.to_string()}))).into_response();
    }

    // Route
    let selected = match state.router().select_upstream(&model_name).await {
        Ok(s) => s,
        Err(e) => {
            let status = match &e {
                xplan_core::RouterError::NoRouteFound(_) => StatusCode::NOT_FOUND,
                xplan_core::RouterError::AllUpstreamsExhausted(_) => StatusCode::SERVICE_UNAVAILABLE,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            return (status, Json(serde_json::json!({"error": e.to_string()}))).into_response();
        }
    };

    let upstream_key = match get_decrypted_key(&state, selected.upstream_key_id).await {
        Ok(k) => k,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response();
        }
    };

    let start = Instant::now();

    // For Anthropic-native requests to Anthropic backends, pass through directly
    // For Anthropic-native requests to OpenAI backends, we'd need conversion (not V1)
    if selected.api_format != "anthropic" {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "error": "Model not available via Anthropic API format. Use /v1/chat/completions."
        }))).into_response();
    }

    // Pass through to Anthropic with model name replaced
    let mut forwarded_body = body.clone();
    forwarded_body["model"] = serde_json::Value::String(selected.upstream_model_name.clone());

    let url = format!("{}/v1/messages", selected.base_url.trim_end_matches('/'));
    let client_http = reqwest::Client::new();

    let mut req_builder = client_http
        .post(&url)
        .header("x-api-key", &upstream_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&forwarded_body);

    let resp = match req_builder.send().await {
        Ok(r) => r,
        Err(e) => {
            return (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"error": e.to_string()}))).into_response();
        }
    };

    let status_code = resp.status().as_u16();
    let latency_ms = start.elapsed().as_millis() as i32;

    if is_stream {
        // Forward stream directly
        let body_stream = axum::body::Body::from_stream(resp.bytes_stream());
        let mut response = Response::new(body_stream);
        response.headers_mut().insert("content-type", "text/event-stream".parse().unwrap());
        response
    } else {
        let resp_body: serde_json::Value = match resp.json().await {
            Ok(b) => b,
            Err(e) => {
                return (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"error": e.to_string()}))).into_response();
            }
        };

        let usage = xplan_provider::TokenUsage {
            input_tokens: resp_body["usage"]["input_tokens"].as_i64().unwrap_or(0) as i32,
            output_tokens: resp_body["usage"]["output_tokens"].as_i64().unwrap_or(0) as i32,
            cache_read_tokens: resp_body["usage"]["cache_read_input_tokens"].as_i64().unwrap_or(0) as i32,
            cache_write_tokens: resp_body["usage"]["cache_creation_input_tokens"].as_i64().unwrap_or(0) as i32,
        };

        let is_success = status_code < 400;
        state.quality().record(selected.provider_model_id, is_success, latency_ms as u32);

        let cost = state.billing().calculate_cost(selected.provider_model_id, &usage).await.unwrap_or(0);

        let pool = state.pool().clone();
        let log = UsageLogInsert {
            client_key_id: client.id,
            upstream_key_id: selected.upstream_key_id,
            provider_model_id: selected.provider_model_id,
            model_name,
            provider_name: get_provider_name(&state, selected.provider_id).await,
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
            cache_read_tokens: usage.cache_read_tokens,
            cache_write_tokens: usage.cache_write_tokens,
            cost_cents: cost,
            latency_ms,
            ttft_ms: None,
            status_code: status_code as i32,
            is_success,
            error_type: None,
        };
        tokio::spawn(async move {
            let _ = xplan_db::repo_usage::insert_usage_log(&pool, &log).await;
        });

        let axum_status = StatusCode::from_u16(status_code).unwrap_or(StatusCode::OK);
        (axum_status, Json(resp_body)).into_response()
    }
}
```

- [ ] **Step 4: Implement models list handler**

`crates/xplan-server/src/proxy/models.rs`:
```rust
use axum::{extract::State, Json};
use serde_json::json;

use crate::state::AppState;

pub async fn handle_list_models(State(state): State<AppState>) -> Json<serde_json::Value> {
    let models = xplan_db::repo_model::list_models(state.pool())
        .await
        .unwrap_or_default();

    let model_list: Vec<serde_json::Value> = models
        .into_iter()
        .filter(|m| m.is_enabled)
        .map(|m| json!({
            "id": m.name,
            "object": "model",
            "created": m.created_at.timestamp(),
            "owned_by": "xplan",
        }))
        .collect();

    Json(json!({
        "object": "list",
        "data": model_list,
    }))
}
```

- [ ] **Step 5: Wire proxy router into main.rs**

Add to main.rs after the app Router definition — replace the simple health route with:

```rust
    // In main.rs, replace the router building section:
    let app = axum::Router::new()
        .route("/health", axum::routing::get(|| async { "ok" }))
        .merge(proxy::router(state.clone()))
        .with_state(state);
```

Add `mod proxy;` at the top of main.rs.

- [ ] **Step 6: Verify compilation**

Run: `cargo build`
Expected: successful compilation

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: add proxy handlers for chat/completions, messages, and models"
```

---

## Task 9: Admin API Handlers

**Files:**
- Create: `crates/xplan-server/src/admin/mod.rs`
- Create: `crates/xplan-server/src/admin/providers.rs`
- Create: `crates/xplan-server/src/admin/upstream_keys.rs`
- Create: `crates/xplan-server/src/admin/models.rs`
- Create: `crates/xplan-server/src/admin/client_keys.rs`
- Create: `crates/xplan-server/src/admin/usage.rs`
- Create: `crates/xplan-server/src/admin/dashboard.rs`
- Modify: `crates/xplan-server/src/main.rs`

- [ ] **Step 1: Admin router module**

`crates/xplan-server/src/admin/mod.rs`:
```rust
pub mod providers;
pub mod upstream_keys;
pub mod models;
pub mod client_keys;
pub mod usage;
pub mod dashboard;

use axum::{middleware, routing::{get, post, put, delete}, Router};
use crate::state::AppState;
use crate::middleware::auth::admin_auth;

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        // Providers
        .route("/admin/api/providers", get(providers::list).post(providers::create))
        .route("/admin/api/providers/{id}", put(providers::update).delete(providers::remove))
        // Upstream keys
        .route("/admin/api/upstream-keys", get(upstream_keys::list).post(upstream_keys::create))
        .route("/admin/api/upstream-keys/{id}", delete(upstream_keys::remove))
        // Models
        .route("/admin/api/models", get(models::list_models).post(models::create_model))
        .route("/admin/api/models/{id}", delete(models::delete_model))
        .route("/admin/api/provider-models", get(models::list_provider_models).post(models::create_provider_model))
        .route("/admin/api/key-model-access", post(models::create_key_model_access))
        // Client keys
        .route("/admin/api/client-keys", get(client_keys::list).post(client_keys::create))
        .route("/admin/api/client-keys/{id}", delete(client_keys::remove))
        // Usage
        .route("/admin/api/usage", get(usage::query))
        .route("/admin/api/usage/summary", get(usage::summary))
        // Dashboard
        .route("/admin/api/dashboard", get(dashboard::overview))
        .route_layer(middleware::from_fn_with_state(state, admin_auth))
}
```

- [ ] **Step 2: Provider admin handlers**

`crates/xplan-server/src/admin/providers.rs`:
```rust
use axum::{extract::{Path, State}, http::StatusCode, Json};
use serde::Deserialize;
use uuid::Uuid;
use xplan_db::{models::ApiFormat, repo_provider};

use crate::state::AppState;

pub async fn list(State(state): State<AppState>) -> Json<serde_json::Value> {
    let providers = repo_provider::list_providers(state.pool()).await.unwrap_or_default();
    Json(serde_json::json!(providers))
}

#[derive(Deserialize)]
pub struct CreateProviderReq {
    name: String,
    base_url: String,
    api_format: ApiFormat,
}

pub async fn create(
    State(state): State<AppState>,
    Json(body): Json<CreateProviderReq>,
) -> (StatusCode, Json<serde_json::Value>) {
    match repo_provider::create_provider(state.pool(), &body.name, &body.base_url, body.api_format).await {
        Ok(p) => (StatusCode::CREATED, Json(serde_json::json!(p))),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e.to_string()}))),
    }
}

#[derive(Deserialize)]
pub struct UpdateProviderReq {
    name: String,
    base_url: String,
    api_format: ApiFormat,
    is_enabled: bool,
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateProviderReq>,
) -> (StatusCode, Json<serde_json::Value>) {
    match repo_provider::update_provider(state.pool(), id, &body.name, &body.base_url, body.api_format, body.is_enabled).await {
        Ok(Some(p)) => (StatusCode::OK, Json(serde_json::json!(p))),
        Ok(None) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Not found"}))),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e.to_string()}))),
    }
}

pub async fn remove(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> StatusCode {
    match repo_provider::delete_provider(state.pool(), id).await {
        Ok(true) => StatusCode::NO_CONTENT,
        Ok(false) => StatusCode::NOT_FOUND,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
```

- [ ] **Step 3: Upstream key admin handlers**

`crates/xplan-server/src/admin/upstream_keys.rs`:
```rust
use axum::{extract::{Path, Query, State}, http::StatusCode, Json};
use serde::Deserialize;
use uuid::Uuid;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use aes_gcm::aead::Aead;
use rand::RngCore;
use xplan_db::repo_upstream_key;

use crate::state::AppState;

pub async fn list(
    State(state): State<AppState>,
    Query(params): Query<ListParams>,
) -> Json<serde_json::Value> {
    let keys = repo_upstream_key::list_upstream_keys(state.pool(), params.provider_id).await.unwrap_or_default();
    // Don't return encrypted keys in list
    let sanitized: Vec<serde_json::Value> = keys.into_iter().map(|k| serde_json::json!({
        "id": k.id,
        "provider_id": k.provider_id,
        "alias": k.alias,
        "is_enabled": k.is_enabled,
        "status": k.status,
        "created_at": k.created_at,
        "updated_at": k.updated_at,
    })).collect();
    Json(serde_json::json!(sanitized))
}

#[derive(Deserialize)]
pub struct ListParams {
    provider_id: Option<Uuid>,
}

#[derive(Deserialize)]
pub struct CreateUpstreamKeyReq {
    provider_id: Uuid,
    alias: String,
    api_key: String,
}

pub async fn create(
    State(state): State<AppState>,
    Json(body): Json<CreateUpstreamKeyReq>,
) -> (StatusCode, Json<serde_json::Value>) {
    let encrypted = match encrypt_key(&body.api_key, state.encryption_key()) {
        Ok(e) => e,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))),
    };

    match repo_upstream_key::create_upstream_key(state.pool(), body.provider_id, &body.alias, &encrypted).await {
        Ok(k) => (StatusCode::CREATED, Json(serde_json::json!({
            "id": k.id,
            "provider_id": k.provider_id,
            "alias": k.alias,
            "status": k.status,
        }))),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e.to_string()}))),
    }
}

pub async fn remove(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> StatusCode {
    match repo_upstream_key::delete_upstream_key(state.pool(), id).await {
        Ok(true) => StatusCode::NO_CONTENT,
        Ok(false) => StatusCode::NOT_FOUND,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn encrypt_key(plaintext: &str, key: &[u8]) -> anyhow::Result<Vec<u8>> {
    let cipher = Aes256Gcm::new_from_slice(key)?;
    let mut nonce_bytes = [0u8; 12];
    rand::rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher.encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))?;
    let mut result = nonce_bytes.to_vec();
    result.extend(ciphertext);
    Ok(result)
}
```

- [ ] **Step 4: Model admin handlers**

`crates/xplan-server/src/admin/models.rs`:
```rust
use axum::{extract::{Path, Query, State}, http::StatusCode, Json};
use serde::Deserialize;
use uuid::Uuid;
use xplan_db::repo_model;

use crate::state::AppState;

pub async fn list_models(State(state): State<AppState>) -> Json<serde_json::Value> {
    let models = repo_model::list_models(state.pool()).await.unwrap_or_default();
    Json(serde_json::json!(models))
}

#[derive(Deserialize)]
pub struct CreateModelReq { name: String }

pub async fn create_model(
    State(state): State<AppState>,
    Json(body): Json<CreateModelReq>,
) -> (StatusCode, Json<serde_json::Value>) {
    match repo_model::create_model(state.pool(), &body.name).await {
        Ok(m) => (StatusCode::CREATED, Json(serde_json::json!(m))),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e.to_string()}))),
    }
}

pub async fn delete_model(State(state): State<AppState>, Path(id): Path<Uuid>) -> StatusCode {
    match repo_model::delete_model(state.pool(), id).await {
        Ok(true) => StatusCode::NO_CONTENT,
        Ok(false) => StatusCode::NOT_FOUND,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

#[derive(Deserialize)]
pub struct ListProviderModelsParams { model_id: Option<Uuid> }

pub async fn list_provider_models(
    State(state): State<AppState>,
    Query(params): Query<ListProviderModelsParams>,
) -> Json<serde_json::Value> {
    let pms = repo_model::list_provider_models(state.pool(), params.model_id).await.unwrap_or_default();
    Json(serde_json::json!(pms))
}

#[derive(Deserialize)]
pub struct CreateProviderModelReq {
    provider_id: Uuid,
    model_id: Uuid,
    upstream_model_name: String,
    input_price_per_mtok: i32,
    output_price_per_mtok: i32,
    cache_read_price_per_mtok: i32,
    cache_write_price_per_mtok: i32,
}

pub async fn create_provider_model(
    State(state): State<AppState>,
    Json(body): Json<CreateProviderModelReq>,
) -> (StatusCode, Json<serde_json::Value>) {
    match repo_model::create_provider_model(
        state.pool(), body.provider_id, body.model_id, &body.upstream_model_name,
        body.input_price_per_mtok, body.output_price_per_mtok,
        body.cache_read_price_per_mtok, body.cache_write_price_per_mtok,
    ).await {
        Ok(pm) => (StatusCode::CREATED, Json(serde_json::json!(pm))),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e.to_string()}))),
    }
}

#[derive(Deserialize)]
pub struct CreateKeyModelAccessReq {
    upstream_key_id: Uuid,
    provider_model_id: Uuid,
    priority: i32,
    weight: i32,
}

pub async fn create_key_model_access(
    State(state): State<AppState>,
    Json(body): Json<CreateKeyModelAccessReq>,
) -> (StatusCode, Json<serde_json::Value>) {
    match repo_model::create_key_model_access(
        state.pool(), body.upstream_key_id, body.provider_model_id, body.priority, body.weight,
    ).await {
        Ok(kma) => (StatusCode::CREATED, Json(serde_json::json!(kma))),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e.to_string()}))),
    }
}
```

- [ ] **Step 5: Client key admin handlers**

`crates/xplan-server/src/admin/client_keys.rs`:
```rust
use axum::{extract::{Path, State}, http::StatusCode, Json};
use serde::Deserialize;
use uuid::Uuid;
use xplan_core::auth::hash_key;
use xplan_db::repo_client_key;

use crate::state::AppState;

pub async fn list(State(state): State<AppState>) -> Json<serde_json::Value> {
    let keys = repo_client_key::list_client_keys(state.pool()).await.unwrap_or_default();
    Json(serde_json::json!(keys))
}

#[derive(Deserialize)]
pub struct CreateClientKeyReq {
    name: String,
    access_all_models: Option<bool>,
    rate_limit_rpm: Option<i32>,
}

pub async fn create(
    State(state): State<AppState>,
    Json(body): Json<CreateClientKeyReq>,
) -> (StatusCode, Json<serde_json::Value>) {
    // Generate a new API key
    let raw_key = format!("sk-xplan-{}", generate_random_key());
    let key_hash = hash_key(&raw_key);
    let key_prefix = &raw_key[..12];

    match repo_client_key::create_client_key(
        state.pool(),
        &body.name,
        &key_hash,
        key_prefix,
        body.access_all_models.unwrap_or(true),
        body.rate_limit_rpm,
    ).await {
        Ok(k) => (StatusCode::CREATED, Json(serde_json::json!({
            "id": k.id,
            "name": k.name,
            "key": raw_key,  // Only shown once at creation
            "key_prefix": k.key_prefix,
            "access_all_models": k.access_all_models,
        }))),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e.to_string()}))),
    }
}

pub async fn remove(State(state): State<AppState>, Path(id): Path<Uuid>) -> StatusCode {
    match repo_client_key::delete_client_key(state.pool(), id).await {
        Ok(true) => StatusCode::NO_CONTENT,
        Ok(false) => StatusCode::NOT_FOUND,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn generate_random_key() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 24];
    rand::rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}
```

- [ ] **Step 6: Usage query handler**

`crates/xplan-server/src/admin/usage.rs`:
```rust
use axum::{extract::{Query, State}, Json};
use chrono::{DateTime, Utc, Duration};
use serde::Deserialize;
use xplan_db::repo_usage;

use crate::state::AppState;

#[derive(Deserialize)]
pub struct UsageQueryParams {
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
    model: Option<String>,
    provider: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

pub async fn query(
    State(state): State<AppState>,
    Query(params): Query<UsageQueryParams>,
) -> Json<serde_json::Value> {
    let from = params.from.unwrap_or_else(|| Utc::now() - Duration::days(7));
    let to = params.to.unwrap_or_else(Utc::now);
    let limit = params.limit.unwrap_or(100).min(1000);
    let offset = params.offset.unwrap_or(0);

    let logs = repo_usage::query_usage(
        state.pool(), from, to, params.model.as_deref(), params.provider.as_deref(), limit, offset,
    ).await.unwrap_or_default();

    Json(serde_json::json!(logs))
}

pub async fn summary(
    State(state): State<AppState>,
    Query(params): Query<UsageQueryParams>,
) -> Json<serde_json::Value> {
    let from = params.from.unwrap_or_else(|| Utc::now() - Duration::days(7));
    let to = params.to.unwrap_or_else(Utc::now);

    let summaries = repo_usage::usage_summary(state.pool(), from, to).await.unwrap_or_default();
    Json(serde_json::json!(summaries))
}
```

- [ ] **Step 7: Dashboard handler**

`crates/xplan-server/src/admin/dashboard.rs`:
```rust
use axum::{extract::State, Json};
use chrono::{Utc, Duration};
use xplan_db::repo_usage;

use crate::state::AppState;

pub async fn overview(State(state): State<AppState>) -> Json<serde_json::Value> {
    let now = Utc::now();
    let day_ago = now - Duration::days(1);

    let summary = repo_usage::usage_summary(state.pool(), day_ago, now).await.unwrap_or_default();

    let total_requests: i64 = summary.iter().map(|s| s.total_requests).sum();
    let total_cost: i64 = summary.iter().map(|s| s.total_cost_cents).sum();
    let total_tokens: i64 = summary.iter().map(|s| s.total_input_tokens + s.total_output_tokens).sum();

    Json(serde_json::json!({
        "period": "last_24h",
        "total_requests": total_requests,
        "total_cost_cents": total_cost,
        "total_tokens": total_tokens,
        "by_model": summary,
    }))
}
```

- [ ] **Step 8: Wire admin router into main.rs**

Add `mod admin;` to main.rs and merge admin router:

```rust
    let app = axum::Router::new()
        .route("/health", axum::routing::get(|| async { "ok" }))
        .merge(proxy::router(state.clone()))
        .merge(admin::router(state.clone()))
        .with_state(state);
```

- [ ] **Step 9: Verify compilation**

Run: `cargo build`
Expected: successful compilation

- [ ] **Step 10: Commit**

```bash
git add -A
git commit -m "feat: add admin API handlers for providers, keys, models, usage, and dashboard"
```

---

## Task 10: Integration Test + End-to-End Verification

**Files:**
- Create: `tests/integration_test.rs` (or use cargo test in workspace)
- Modify: `crates/xplan-server/src/main.rs` (add migration on startup)

- [ ] **Step 1: Add auto-migration on startup**

In `main.rs`, after connecting to DB:
```rust
    // Run migrations
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await?;
    tracing::info!("Migrations applied");
```

- [ ] **Step 2: Manual integration test**

Start dependencies:
```bash
# Ensure PG is running with xplan database
# Ensure Redis is running on port 6379 with password dev
redis-cli -a dev ping
```

Run the server:
```bash
cargo run --bin xplan-server
```

Create a provider, model, key setup via curl:
```bash
ADMIN="xplan-admin-dev-token"

# Create provider
curl -X POST http://localhost:3000/admin/api/providers \
  -H "Authorization: Bearer $ADMIN" \
  -H "Content-Type: application/json" \
  -d '{"name":"openai","base_url":"https://api.openai.com","api_format":"openai_compatible"}'

# Create model
curl -X POST http://localhost:3000/admin/api/models \
  -H "Authorization: Bearer $ADMIN" \
  -H "Content-Type: application/json" \
  -d '{"name":"gpt-4o-mini"}'

# Create client key
curl -X POST http://localhost:3000/admin/api/client-keys \
  -H "Authorization: Bearer $ADMIN" \
  -H "Content-Type: application/json" \
  -d '{"name":"test-client"}'
# → note the "key" in response

# List models
curl http://localhost:3000/v1/models \
  -H "Authorization: Bearer <client-key-from-above>"
```

Expected: models list returns the created model.

- [ ] **Step 3: Verify health endpoint**

```bash
curl http://localhost:3000/health
```
Expected: `ok`

- [ ] **Step 4: Commit final adjustments**

```bash
git add -A
git commit -m "feat: add auto-migration and verify end-to-end flow"
```

---

## Task 11: Frontend (React + Vite Admin Panel)

> This task is intentionally high-level. The frontend is a separate sub-project that follows after the backend is stable and tested. It should get its own detailed plan when ready.

**Files:**
- Create: `frontend/` (React + Vite project)

- [ ] **Step 1: Scaffold frontend project**

```bash
cd /Users/wellxie/projects/claw-works/xplan
bun create vite frontend --template react-ts
cd frontend && bun install
```

- [ ] **Step 2: Install dependencies**

```bash
cd frontend
bun add @tanstack/react-query axios recharts lucide-react
bun add -d tailwindcss @tailwindcss/vite
```

- [ ] **Step 3: Create basic pages**

Pages needed:
- Dashboard (overview stats, charts)
- Providers (CRUD table)
- Upstream Keys (CRUD table with encrypted key handling)
- Models (model + provider_model mapping)
- Client Keys (CRUD + key generation)
- Usage Logs (filterable table)
- Quality Stats (charts per model+provider)

- [ ] **Step 4: Connect to admin API**

Create API client that uses the admin token and calls `/admin/api/*` endpoints.

- [ ] **Step 5: Serve from xplan-server**

Add static file serving to main.rs using `tower_http::services::ServeDir`:
```rust
use tower_http::services::ServeDir;

// In router setup:
.nest_service("/admin", ServeDir::new("frontend/dist"))
```

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: add frontend admin panel scaffold"
```

---

## Summary of Implementation Order

1. **Workspace + Config** — project foundation
2. **Database Schema** — data model
3. **Cache Layer** — moka + Redis
4. **Provider Adapters** — OpenAI + Anthropic
5. **Core Domain** — auth, router, billing, quality
6. **DB Repositories** — CRUD operations
7. **Server + Middleware** — AppState, auth
8. **Proxy Handlers** — the actual API gateway
9. **Admin Handlers** — management API
10. **Integration Test** — end-to-end verification
11. **Frontend** — admin panel (separate phase)
