use serde::{Deserialize, Serialize};
use uuid::Uuid;
use xplan_db::models::ApiFormat;

// ── Providers ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateProviderBody {
    pub name: String,
    pub base_url: String,
    pub api_format: ApiFormat,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProviderBody {
    pub name: String,
    pub base_url: String,
    pub api_format: ApiFormat,
    pub is_enabled: bool,
}

// ── Upstream Keys ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ListUpstreamKeysQuery {
    pub provider_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct CreateUpstreamKeyBody {
    pub provider_id: Uuid,
    pub alias: String,
    pub api_key: String,
}

#[derive(Debug, Serialize)]
pub struct UpstreamKeyResponse {
    pub id: Uuid,
    pub provider_id: Uuid,
    pub alias: String,
    pub is_enabled: bool,
    pub status: xplan_db::models::KeyStatus,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

// ── Models ─────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateModelBody {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateModelBody {
    pub name: String,
    pub is_enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProviderModelBody {
    pub upstream_model_name: String,
    pub input_price_per_mtok: i32,
    pub output_price_per_mtok: i32,
    pub cache_read_price_per_mtok: i32,
    pub cache_write_price_per_mtok: i32,
    #[serde(default)]
    pub config: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct ListProviderModelsQuery {
    pub model_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct CreateProviderModelBody {
    pub provider_id: Uuid,
    pub model_id: Uuid,
    pub upstream_model_name: String,
    pub input_price_per_mtok: i32,
    pub output_price_per_mtok: i32,
    pub cache_read_price_per_mtok: i32,
    pub cache_write_price_per_mtok: i32,
    #[serde(default)]
    pub config: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct CreateKeyModelAccessBody {
    pub upstream_key_id: Uuid,
    pub provider_model_id: Uuid,
    pub priority: i32,
    pub weight: i32,
}

// ── Client Keys ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateClientKeyBody {
    pub name: String,
    pub access_all_models: Option<bool>,
    pub rate_limit_rpm: Option<i32>,
    pub role: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CreateClientKeyResponse {
    pub id: Uuid,
    pub name: String,
    pub key: String,
    pub key_prefix: String,
    pub access_all_models: bool,
    pub is_enabled: bool,
    pub rate_limit_rpm: Option<i32>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

// ── Usage ──────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct UsageQuery {
    pub from: Option<chrono::DateTime<chrono::Utc>>,
    pub to: Option<chrono::DateTime<chrono::Utc>>,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct UsageSummaryQuery {
    pub from: Option<chrono::DateTime<chrono::Utc>>,
    pub to: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct UsageBreakdownQuery {
    pub from: Option<chrono::DateTime<chrono::Utc>>,
    pub to: Option<chrono::DateTime<chrono::Utc>>,
}
