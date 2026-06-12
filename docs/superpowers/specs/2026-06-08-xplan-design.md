# xplan - LLM API Gateway Design Spec

## Overview

xplan is a self-hosted LLM API gateway for commercial operations, similar to OpenRouter. It aggregates multiple upstream providers and API keys behind standardized interfaces, with intelligent routing, load balancing, failover, token-level billing, and usage auditing.

## Goals

- Aggregate multiple LLM providers (OpenAI-compatible including Gemini and Bedrock, + Anthropic) behind a unified gateway
- Expose OpenAI-compatible `/v1/chat/completions` and Anthropic-native `/v1/messages` interfaces
- Support virtual model names (e.g., "xplan-flash") that route across multiple providers
- Track token usage (input/output/cache_read/cache_write) per request for auditing
- Weighted load balancing with priority-based failover
- Service quality monitoring per model+provider
- Time-window based quota management (e.g., "100 requests per 5 hours")
- Web admin panel for key management and usage dashboards

## Non-Goals (V1)

- Multi-tenant SaaS (no user registration, billing plans)
- OAuth/Agent-based upstream authentication (reserved for future)
- Smart routing based on cost/latency optimization (architecture supports it, not implemented in V1)
- Circuit breaker with automatic recovery (reserved, quality_factor in routing provides soft degradation)

## Architecture

### High-Level

```
┌─────────────────────────────────────────────────────┐
│                    xplan binary                       │
├─────────────┬──────────────────┬────────────────────┤
│  Proxy Layer │  Admin API Layer │  Static File Serve │
│  /v1/chat/* │  /admin/api/*    │  /admin/*  (SPA)   │
│  /v1/messages│                  │                    │
├─────────────┴──────────────────┴────────────────────┤
│                  Domain Layer                         │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌────────┐ │
│  │Key Pool  │ │Router/LB │ │Billing   │ │Provider│ │
│  │Manager   │ │Engine    │ │Engine    │ │Adapter │ │
│  └──────────┘ └──────────┘ └──────────┘ └────────┘ │
├─────────────────────────────────────────────────────┤
│              Infrastructure Layer                     │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌────────┐ │
│  │PostgreSQL│ │Redis     │ │HTTP Client│ │Config  │ │
│  │(sqlx)    │ │(deadpool)│ │(reqwest)  │ │(toml)  │ │
│  └──────────┘ └──────────┘ └──────────┘ └────────┘ │
└─────────────────────────────────────────────────────┘
```

### Workspace Structure

```
xplan/
├── Cargo.toml          (workspace)
├── crates/
│   ├── xplan-server/   (binary, axum entry point)
│   ├── xplan-core/     (domain: router, billing, quality)
│   ├── xplan-provider/ (provider adapters: OpenAI-compat, Anthropic)
│   ├── xplan-db/       (PG schema, migrations, queries)
│   └── xplan-cache/    (cache abstraction: moka local + Redis)
├── frontend/           (React + Vite SPA)
├── config/             (example config files)
└── migrations/         (SQL migrations)
```

### Request Flow

```
Client request (model="xplan-flash", key="sk-xplan-xxx")
  │
  ├─ 1. Extract API key (Authorization: Bearer / x-api-key header)
  ├─ 2. Authenticate client key (hash compare via cache chain)
  ├─ 3. Check client_model_access: is this client allowed to use "xplan-flash"?
  │
  ├─ 4. Resolve model: models table → model_id
  ├─ 5. Find candidates: provider_models → key_model_access → upstream_keys
  │     Filter: enabled + quota not exhausted
  │
  ├─ 6. Route selection:
  │     Group by priority (ASC) → within group:
  │       score = weight × quality_factor
  │       quality_factor = success_rate × (1 / normalized_latency)
  │     Weighted random selection
  │
  ├─ 7. Forward request via ProviderAdapter
  │     Replace model name with upstream_model_name
  │
  ├─ 8. On response:
  │     Extract usage (input/output/cache tokens)
  │     Calculate cost via BillingEngine
  │     Record usage_log (async batch write)
  │     Update QualityMonitor sliding window
  │     Update quota counter (Redis INCRBY)
  │
  └─ 9. Return response to client
       (On failure: mark key degraded, retry next candidate)
```

## Data Model

### providers

| Column | Type | Description |
|--------|------|-------------|
| id | uuid | PK |
| name | varchar | "openai", "anthropic", "deepseek" |
| base_url | varchar | "https://api.openai.com" |
| api_format | enum | openai_compatible \| anthropic \| bedrock |
| auth_type | varchar | "api_key" (future: "oauth", "agent_client") |
| auth_config | jsonb | Future-proof auth configuration |
| is_enabled | bool | |
| created_at / updated_at | timestamptz | |

### upstream_keys

| Column | Type | Description |
|--------|------|-------------|
| id | uuid | PK |
| provider_id | uuid FK | |
| alias | varchar | Human-readable name |
| api_key_encrypted | bytea | AES-256-GCM encrypted |
| is_enabled | bool | |
| status | enum | healthy \| degraded \| disabled |
| created_at / updated_at | timestamptz | |

### key_quotas

| Column | Type | Description |
|--------|------|-------------|
| id | uuid | PK |
| upstream_key_id | uuid FK | |
| quota_type | enum | rpm \| rpd \| requests_per_window \| tokens_per_window |
| limit_value | int | Upper limit |
| window_seconds | int | Time window (e.g., 18000 for 5h) |
| created_at / updated_at | timestamptz | |

Runtime quota state lives in Redis (`key_quota:{key_id}:{quota_id}` → INCRBY + EXPIRE).

### models

| Column | Type | Description |
|--------|------|-------------|
| id | uuid | PK |
| name | varchar | External name: "xplan-flash", "gpt-4o" |
| is_enabled | bool | |
| created_at / updated_at | timestamptz | |

### provider_models

| Column | Type | Description |
|--------|------|-------------|
| id | uuid | PK |
| provider_id | uuid FK | |
| model_id | uuid FK | |
| upstream_model_name | varchar | Actual upstream name (e.g., "deepseek-chat") |
| input_price_per_mtok | int | Price per 1M input tokens (cents) |
| output_price_per_mtok | int | Price per 1M output tokens (cents) |
| cache_read_price_per_mtok | int | |
| cache_write_price_per_mtok | int | |
| updated_at | timestamptz | |

### key_model_access

| Column | Type | Description |
|--------|------|-------------|
| id | uuid | PK |
| upstream_key_id | uuid FK | |
| provider_model_id | uuid FK | |
| priority | int | Lower = higher priority |
| weight | int | Load balancing weight within same priority |
| is_enabled | bool | |
| created_at / updated_at | timestamptz | |

### client_keys

| Column | Type | Description |
|--------|------|-------------|
| id | uuid | PK |
| name | varchar | "team-a", "bot-service" |
| key_hash | varchar | Hashed, never store plaintext |
| key_prefix | varchar | First 8 chars for identification (e.g., "sk-xplan-") |
| access_all_models | bool | If true, skip model access check |
| is_enabled | bool | |
| rate_limit_rpm | int? | Client-level rate limit |
| created_at / updated_at | timestamptz | |

### client_model_access

| Column | Type | Description |
|--------|------|-------------|
| id | uuid | PK |
| client_key_id | uuid FK | |
| model_id | uuid FK | |
| is_enabled | bool | |
| created_at | timestamptz | |

### usage_logs

| Column | Type | Description |
|--------|------|-------------|
| id | bigserial | PK |
| client_key_id | uuid FK | |
| upstream_key_id | uuid FK | |
| provider_model_id | uuid FK | |
| model_name | varchar | Denormalized for query convenience |
| provider_name | varchar | Denormalized |
| input_tokens | int | |
| output_tokens | int | |
| cache_read_tokens | int | |
| cache_write_tokens | int | |
| cost_cents | int | Calculated cost |
| latency_ms | int | Total request duration |
| ttft_ms | int? | Time to first token (streaming) |
| status_code | int | Upstream response code |
| is_success | bool | |
| error_type | varchar? | "rate_limit", "timeout", "server_error" |
| created_at | timestamptz | |

Partitioned by month.

### quality_stats

| Column | Type | Description |
|--------|------|-------------|
| id | uuid | PK |
| provider_model_id | uuid FK | |
| period_start | timestamptz | |
| period_type | enum | hourly \| daily |
| total_requests | int | |
| success_count | int | |
| error_count | int | |
| avg_latency_ms | int | |
| p95_latency_ms | int | |
| avg_ttft_ms | int? | |
| total_tokens | bigint | |
| updated_at | timestamptz | |

## Domain Modules

### Provider Adapter

```rust
trait ProviderAdapter: Send + Sync {
    async fn chat_completion(&self, req: UpstreamRequest) -> Result<UpstreamResponse>;
    async fn chat_completion_stream(&self, req: UpstreamRequest) -> Result<StreamResponse>;
}
```

Three implementations:
- `OpenAiCompatAdapter` — covers OpenAI, DeepSeek, Groq, Together, Gemini, etc.
- `AnthropicAdapter` — Anthropic-specific format with extended thinking support
- `BedrockConverseAdapter` — AWS Bedrock Converse API format, API key authentication

Responsibilities:
- Transform internal unified request to upstream API format
- Transform upstream response to internal unified format
- Replace model name (virtual → upstream)
- Extract usage from response

### Router Engine

Routing algorithm:
1. Resolve model_id from requested model name
2. Find all key_model_access entries for this model
3. Filter: enabled + quota not exhausted + key status != disabled
4. Group by priority (ascending)
5. For highest available priority group: `score = weight × quality_factor`
6. Weighted random selection based on scores
7. On failure: mark degraded, try next candidate in group, then next priority group
8. All exhausted: return 503

### Quota Tracker

- Redis-based: `INCRBY` + `EXPIRE` for atomic increment with auto-reset
- Key format: `xplan:quota:{upstream_key_id}:{quota_id}`
- On first request in window: SET with EXPIRE = window_seconds
- Check before routing: GET and compare against limit_value
- Lazy reset: Redis TTL handles expiration automatically

### Billing Engine

```
cost = (input_tokens × input_price_per_mtok / 1_000_000)
     + (output_tokens × output_price_per_mtok / 1_000_000)
     + (cache_read_tokens × cache_read_price_per_mtok / 1_000_000)
     + (cache_write_tokens × cache_write_price_per_mtok / 1_000_000)
```

All prices in cents per million tokens. Cost stored as integer cents.

Usage log write strategy:
- Non-streaming: calculate after response, async batch write via channel
- Streaming: accumulate usage during stream, write after stream ends
- Batch insert (every 1s or 100 records) to reduce DB pressure

### Quality Monitor

- In-memory sliding window per provider_model (1-hour window, 1-min buckets)
- Tracks: success/failure count, total latency, request count per bucket
- Provides real-time `success_rate` and `avg_latency` to Router Engine
- Periodically aggregates to `quality_stats` table for admin dashboard
- Soft degradation: quality_factor naturally reduces traffic to slow/failing upstreams

## Caching Architecture

Three-tier cache chain: Local (moka) → Redis → PostgreSQL

| Data | Local (moka) | Redis | PG |
|------|-------------|-------|-----|
| Client key auth | hash cache, TTL 60s | Full key_hash set | Source of truth |
| Route table | Full in-memory, periodic refresh | Change notification (pub/sub) | Source of truth |
| Quota counters | — | INCRBY + EXPIRE (authoritative) | Periodic snapshot |
| Price table | Full in-memory, TTL 5min | — | Source of truth |
| Quality metrics | Sliding window (real-time) | — | Aggregated writes |

Cache invalidation:
- Admin mutations → Redis pub/sub notify → local cache eviction
- TTL as fallback (prevents stale data if pub/sub message lost)
- Process startup: load from PG → warm local + Redis

## API Design

### Proxy Endpoints (client-facing)

| Method | Path | Description |
|--------|------|-------------|
| POST | /v1/chat/completions | OpenAI-compatible chat |
| POST | /v1/messages | Anthropic-native messages |
| GET | /v1/models | List available models |

Authentication: `Authorization: Bearer sk-xplan-xxx` OR `x-api-key: sk-xplan-xxx`

### Admin Endpoints

| Method | Path | Description |
|--------|------|-------------|
| CRUD | /admin/api/providers | Provider management |
| CRUD | /admin/api/upstream-keys | Upstream key management |
| CRUD | /admin/api/models | Model definitions |
| CRUD | /admin/api/provider-models | Model-provider mapping + pricing |
| CRUD | /admin/api/client-keys | Client key management |
| GET | /admin/api/usage | Usage query (filter by time/model/provider) |
| GET | /admin/api/quality | Service quality stats |
| GET | /admin/api/dashboard | Aggregated dashboard data |

Admin authentication: static admin token from config file (`admin_token` in config.toml). Simple Bearer token check. No session management in V1.

## Technology Stack

| Component | Choice | Rationale |
|-----------|--------|-----------|
| Language | Rust | High performance, low latency for proxy gateway |
| Web framework | axum | Mature, tower middleware, great performance |
| Database | PostgreSQL + sqlx | Compile-time SQL checks, async |
| Cache (local) | moka | High-perf async LRU/TTL cache |
| Cache (shared) | Redis (redis-rs + deadpool) | Atomic counters, pub/sub, connection pool |
| HTTP client | reqwest | Mature, streaming support |
| Serialization | serde + serde_json | Standard |
| Config | toml (config crate) | Rust ecosystem standard |
| Migrations | sqlx-cli | Integrated with sqlx |
| Encryption | aes-gcm | Upstream key encryption at rest |
| Frontend | React + Vite | Fast dev, static file deployment |

## Future Extensions (Not V1)

- **OAuth upstream auth**: For providers requiring OAuth flow
- **Agent client auth**: For CLI-based providers (e.g., kiro-cli)
- **Circuit breaker**: Auto-disable provider_model when error rate > threshold, auto-recovery after cooldown
- **Smart routing**: Cost/latency optimization, model capability matching
- **Multi-tenant SaaS**: User registration, billing plans, rate limiting per tenant
- **Streaming token counting**: Real-time token estimation during stream (before final usage report)

## Development Environment

- PostgreSQL: local, user=dev, password=dev
- Redis: local, default port (6379), password=dev
- Rust edition: 2024
- MSRV: latest stable
