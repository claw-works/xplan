use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "api_format", rename_all = "snake_case")]
pub enum ApiFormat {
    OpenaiCompatible,
    Anthropic,
    Bedrock,
    Responses,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "key_status", rename_all = "snake_case")]
pub enum KeyStatus {
    Healthy,
    Degraded,
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "quota_type", rename_all = "snake_case")]
pub enum QuotaType {
    Rpm,
    Rpd,
    RequestsPerWindow,
    TokensPerWindow,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "period_type", rename_all = "snake_case")]
pub enum PeriodType {
    Hourly,
    Daily,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "client_role", rename_all = "snake_case")]
pub enum ClientRole {
    User,
    Admin,
}

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
    pub config: serde_json::Value,
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
    pub role: ClientRole,
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
    pub cost_cents: i64,
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
