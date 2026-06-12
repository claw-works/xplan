use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::{ClientKey, ClientRole};

pub async fn list_client_keys(pool: &PgPool) -> Result<Vec<ClientKey>> {
    let rows = sqlx::query_as::<_, ClientKey>(
        "SELECT id, name, key_hash, key_prefix, access_all_models, is_enabled, rate_limit_rpm, role, created_at, updated_at
         FROM client_keys
         ORDER BY name",
    )
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
    role: ClientRole,
) -> Result<ClientKey> {
    let row = sqlx::query_as::<_, ClientKey>(
        "INSERT INTO client_keys (name, key_hash, key_prefix, access_all_models, rate_limit_rpm, role)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id, name, key_hash, key_prefix, access_all_models, is_enabled, rate_limit_rpm, role, created_at, updated_at",
    )
    .bind(name)
    .bind(key_hash)
    .bind(key_prefix)
    .bind(access_all_models)
    .bind(rate_limit_rpm)
    .bind(role)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn count_admin_keys(pool: &PgPool) -> Result<i64> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM client_keys WHERE role = 'admin'")
        .fetch_one(pool)
        .await?;
    Ok(row.0)
}

pub async fn update_client_key_enabled(pool: &PgPool, id: Uuid, is_enabled: bool) -> Result<()> {
    sqlx::query(
        "UPDATE client_keys SET is_enabled = $2, updated_at = now() WHERE id = $1",
    )
    .bind(id)
    .bind(is_enabled)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete_client_key(pool: &PgPool, id: Uuid) -> Result<bool> {
    let result = sqlx::query("DELETE FROM client_keys WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}
