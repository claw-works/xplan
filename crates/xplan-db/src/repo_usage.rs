use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::UsageLog;

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
    pub cost_cents: i64,
    pub latency_ms: i32,
    pub ttft_ms: Option<i32>,
    pub status_code: i32,
    pub is_success: bool,
    pub error_type: Option<String>,
}

pub async fn insert_usage_log(pool: &PgPool, log: &UsageLogInsert) -> Result<()> {
    sqlx::query(
        "INSERT INTO usage_logs
             (client_key_id, upstream_key_id, provider_model_id,
              model_name, provider_name,
              input_tokens, output_tokens, cache_read_tokens, cache_write_tokens,
              cost_cents, latency_ms, ttft_ms,
              status_code, is_success, error_type)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)",
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
) -> Result<Vec<UsageLog>> {
    let rows = sqlx::query_as::<_, UsageLog>(
        "SELECT id, client_key_id, upstream_key_id, provider_model_id,
                model_name, provider_name,
                input_tokens, output_tokens, cache_read_tokens, cache_write_tokens,
                cost_cents, latency_ms, ttft_ms,
                status_code, is_success, error_type, created_at
         FROM usage_logs
         WHERE created_at >= $1
           AND created_at <= $2
           AND ($3::text IS NULL OR model_name = $3)
           AND ($4::text IS NULL OR provider_name = $4)
         ORDER BY created_at DESC
         LIMIT $5 OFFSET $6",
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

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UsageSummary {
    pub model_name: String,
    pub provider_name: String,
    pub total_requests: i64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub total_cost_cents: i64,
    pub avg_latency_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UsageBreakdown {
    pub group_id: Uuid,
    pub group_name: String,
    pub model_name: String,
    pub provider_name: String,
    pub total_requests: i64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub total_cost_cents: i64,
}

pub async fn usage_summary(
    pool: &PgPool,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<Vec<UsageSummary>> {
    let rows = sqlx::query_as::<_, UsageSummary>(
        "SELECT
             model_name,
             provider_name,
             COUNT(*)::bigint          AS total_requests,
             SUM(input_tokens)::bigint  AS total_input_tokens,
             SUM(output_tokens)::bigint AS total_output_tokens,
             SUM(cost_cents)::bigint    AS total_cost_cents,
             AVG(latency_ms)::double precision AS avg_latency_ms
         FROM usage_logs
         WHERE created_at >= $1 AND created_at <= $2
         GROUP BY model_name, provider_name
         ORDER BY total_requests DESC",
    )
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn usage_by_upstream_key(
    pool: &PgPool,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<Vec<UsageBreakdown>> {
    let rows = sqlx::query_as::<_, UsageBreakdown>(
        r#"
        SELECT
            uk.id as group_id,
            uk.alias as group_name,
            ul.model_name,
            ul.provider_name,
            COUNT(*)::bigint as total_requests,
            COALESCE(SUM(ul.input_tokens), 0)::bigint as total_input_tokens,
            COALESCE(SUM(ul.output_tokens), 0)::bigint as total_output_tokens,
            COALESCE(SUM(ul.cost_cents), 0)::bigint as total_cost_cents
        FROM usage_logs ul
        JOIN upstream_keys uk ON uk.id = ul.upstream_key_id
        WHERE ul.created_at >= $1 AND ul.created_at < $2
        GROUP BY uk.id, uk.alias, ul.model_name, ul.provider_name
        ORDER BY total_requests DESC
        "#,
    )
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn usage_by_client_key(
    pool: &PgPool,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<Vec<UsageBreakdown>> {
    let rows = sqlx::query_as::<_, UsageBreakdown>(
        r#"
        SELECT
            ck.id as group_id,
            ck.name as group_name,
            ul.model_name,
            ul.provider_name,
            COUNT(*)::bigint as total_requests,
            COALESCE(SUM(ul.input_tokens), 0)::bigint as total_input_tokens,
            COALESCE(SUM(ul.output_tokens), 0)::bigint as total_output_tokens,
            COALESCE(SUM(ul.cost_cents), 0)::bigint as total_cost_cents
        FROM usage_logs ul
        JOIN client_keys ck ON ck.id = ul.client_key_id
        WHERE ul.created_at >= $1 AND ul.created_at < $2
        GROUP BY ck.id, ck.name, ul.model_name, ul.provider_name
        ORDER BY total_requests DESC
        "#,
    )
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
