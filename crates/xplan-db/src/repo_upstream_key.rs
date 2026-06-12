use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::{KeyStatus, UpstreamKey};

pub async fn list_upstream_keys(
    pool: &PgPool,
    provider_id: Option<Uuid>,
) -> Result<Vec<UpstreamKey>> {
    let rows = match provider_id {
        Some(pid) => sqlx::query_as::<_, UpstreamKey>(
            "SELECT id, provider_id, alias, api_key_encrypted, is_enabled, status, created_at, updated_at
             FROM upstream_keys
             WHERE provider_id = $1
             ORDER BY alias",
        )
        .bind(pid)
        .fetch_all(pool)
        .await?,
        None => sqlx::query_as::<_, UpstreamKey>(
            "SELECT id, provider_id, alias, api_key_encrypted, is_enabled, status, created_at, updated_at
             FROM upstream_keys
             ORDER BY alias",
        )
        .fetch_all(pool)
        .await?,
    };
    Ok(rows)
}

pub async fn get_upstream_key(pool: &PgPool, id: Uuid) -> Result<Option<UpstreamKey>> {
    let row = sqlx::query_as::<_, UpstreamKey>(
        "SELECT id, provider_id, alias, api_key_encrypted, is_enabled, status, created_at, updated_at
         FROM upstream_keys
         WHERE id = $1",
    )
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
) -> Result<UpstreamKey> {
    let row = sqlx::query_as::<_, UpstreamKey>(
        "INSERT INTO upstream_keys (provider_id, alias, api_key_encrypted)
         VALUES ($1, $2, $3)
         RETURNING id, provider_id, alias, api_key_encrypted, is_enabled, status, created_at, updated_at",
    )
    .bind(provider_id)
    .bind(alias)
    .bind(api_key_encrypted)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn update_upstream_key_status(pool: &PgPool, id: Uuid, status: KeyStatus) -> Result<()> {
    sqlx::query(
        "UPDATE upstream_keys SET status = $2, updated_at = now() WHERE id = $1",
    )
    .bind(id)
    .bind(status)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete_upstream_key(pool: &PgPool, id: Uuid) -> Result<bool> {
    let result = sqlx::query("DELETE FROM upstream_keys WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}
