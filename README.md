# xplan

Self-hosted LLM API gateway for commercial operations. Aggregates multiple providers (OpenAI, Anthropic, Bedrock, Gemini, DeepSeek, etc.) behind unified interfaces with intelligent routing, quota management, token billing, and quality monitoring.

## Features

- **Multi-provider routing** — OpenAI-compatible + Anthropic native + Bedrock Converse API
- **Virtual model names** — map `xplan-flash` to multiple flash models across providers
- **Weighted load balancing** — priority-based failover with quality-aware scoring
- **Quota management** — time-window based (e.g., "100 requests per 5 hours") via Redis
- **Token billing** — per-request cost calculation with input/output/cache token tracking
- **Usage auditing** — full request log with model, provider, tokens, latency, cost
- **Service quality monitoring** — sliding window success rate + latency per model+provider
- **Admin panel** — React SPA for key management, usage dashboard, quality stats

## Quick Start

### Prerequisites

- Rust (stable)
- PostgreSQL (local, user=dev, password=dev)
- Redis (local, port 6379, password=dev)
- Bun (for frontend)

### Run

```bash
# Start the backend (auto-applies migrations)
cargo run --bin xplan-server

# Frontend dev mode (separate terminal)
cd frontend && bun run dev
```

Server starts at `http://localhost:26011`. Frontend dev at `http://localhost:26012`.

### First Run

On first startup, if no admin key exists, the server will auto-create one and print it to the console:

```
===========================================
  No admin key found. Created initial admin key:
  sk-xplan-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
  Save this key! It won't be shown again.
===========================================
```

Use this key to log into the admin panel or call admin API endpoints.

### CLI Commands

```bash
# Start the server
cargo run --bin xplan-server

# Create a new admin key (if you lost the original)
cargo run --bin xplan-server -- create-admin-key
```

### Configuration

All config in `config/default.toml`. Override via environment variables:

```bash
XPLAN__SERVER__PORT=8080
XPLAN__DATABASE__URL="postgres://user:pass@host/db"
XPLAN__REDIS__URL="redis://:password@host:6379"
XPLAN__ENCRYPTION__KEY_HEX="64-char-hex-string"
```

> ⚠️ **Production**: change `encryption.key_hex` to a unique 32-byte hex string (e.g. `openssl rand -hex 32`). The default value in `config/default.toml` is for local development only — using it in production lets anyone with DB access decrypt all stored upstream API keys.

### Docker

A multi-stage `Dockerfile` builds the frontend (Bun) and Rust binary into a single image. The compiled server serves the admin UI from `frontend/dist` at `/admin/`.

```bash
# Pull from GHCR (built by .github/workflows/docker.yml on push to main)
docker pull ghcr.io/claw-works/xplan:latest

# Run
docker run -d --name xplan -p 26011:26011 \
  -e XPLAN__DATABASE__URL="postgres://user:pass@host/xplan" \
  -e XPLAN__REDIS__URL="redis://:password@host:6379" \
  -e XPLAN__ENCRYPTION__KEY_HEX="$(openssl rand -hex 32)" \
  ghcr.io/claw-works/xplan:latest
```

Build locally: `docker build -t xplan .`

## API

### Proxy Endpoints (client-facing)

| Method | Path | Description |
|--------|------|-------------|
| POST | `/v1/chat/completions` | OpenAI-compatible chat |
| POST | `/v1/messages` | Anthropic-native messages |
| GET | `/v1/models` | List available models |

Auth: `Authorization: Bearer sk-xplan-xxx` or `x-api-key: sk-xplan-xxx`

### Admin Endpoints

| Method | Path | Description |
|--------|------|-------------|
| CRUD | `/admin/api/providers` | Provider management |
| CRUD | `/admin/api/upstream-keys` | Upstream key management |
| CRUD | `/admin/api/models` | Model definitions |
| CRUD | `/admin/api/provider-models` | Model-provider mapping + pricing |
| CRUD | `/admin/api/client-keys` | Client key management |
| GET | `/admin/api/usage` | Usage query |
| GET | `/admin/api/dashboard` | Dashboard stats |

Auth: `Authorization: Bearer <admin-key>` (requires admin role)

### Authentication

All API keys are client keys stored in the database with a `role` field (`user` or `admin`). Both proxy and admin endpoints use the same key format (`sk-xplan-xxx`). Admin endpoints additionally require the key to have `role = admin`.

## Architecture

```
xplan/
├── crates/
│   ├── xplan-server/    # axum HTTP server, handlers, middleware
│   ├── xplan-core/      # domain logic: router, billing, quality, auth
│   ├── xplan-provider/  # provider adapters (OpenAI, Anthropic, Bedrock)
│   ├── xplan-db/        # PostgreSQL schema, models, repositories
│   └── xplan-cache/     # moka local cache + Redis (quota, invalidation)
└── frontend/            # React + Vite admin panel
```

## License

MIT
