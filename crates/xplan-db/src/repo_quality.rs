use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::{PeriodType, QualityStat};

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
) -> Result<()> {
    sqlx::query(
        "INSERT INTO quality_stats
             (provider_model_id, period_start, period_type,
              total_requests, success_count, error_count,
              avg_latency_ms, p95_latency_ms, avg_ttft_ms, total_tokens)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
         ON CONFLICT (provider_model_id, period_start, period_type)
         DO UPDATE SET
             total_requests  = EXCLUDED.total_requests,
             success_count   = EXCLUDED.success_count,
             error_count     = EXCLUDED.error_count,
             avg_latency_ms  = EXCLUDED.avg_latency_ms,
             p95_latency_ms  = EXCLUDED.p95_latency_ms,
             avg_ttft_ms     = EXCLUDED.avg_ttft_ms,
             total_tokens    = EXCLUDED.total_tokens,
             updated_at      = now()",
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
) -> Result<Vec<QualityStat>> {
    let rows = match provider_model_id {
        Some(pmid) => sqlx::query_as::<_, QualityStat>(
            "SELECT id, provider_model_id, period_start, period_type,
                    total_requests, success_count, error_count,
                    avg_latency_ms, p95_latency_ms, avg_ttft_ms, total_tokens, updated_at
             FROM quality_stats
             WHERE provider_model_id = $1
               AND period_type = $2
               AND period_start >= $3
               AND period_start <= $4
             ORDER BY period_start",
        )
        .bind(pmid)
        .bind(period_type)
        .bind(from)
        .bind(to)
        .fetch_all(pool)
        .await?,
        None => sqlx::query_as::<_, QualityStat>(
            "SELECT id, provider_model_id, period_start, period_type,
                    total_requests, success_count, error_count,
                    avg_latency_ms, p95_latency_ms, avg_ttft_ms, total_tokens, updated_at
             FROM quality_stats
             WHERE period_type = $1
               AND period_start >= $2
               AND period_start <= $3
             ORDER BY period_start",
        )
        .bind(period_type)
        .bind(from)
        .bind(to)
        .fetch_all(pool)
        .await?,
    };
    Ok(rows)
}
