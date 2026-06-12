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
