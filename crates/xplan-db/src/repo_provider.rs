use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::{ApiFormat, Provider};

pub async fn list_providers(pool: &PgPool) -> Result<Vec<Provider>> {
    let rows = sqlx::query_as::<_, Provider>(
        "SELECT id, name, base_url, api_format, auth_type, auth_config, is_enabled, created_at, updated_at
         FROM providers
         ORDER BY name",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn get_provider(pool: &PgPool, id: Uuid) -> Result<Option<Provider>> {
    let row = sqlx::query_as::<_, Provider>(
        "SELECT id, name, base_url, api_format, auth_type, auth_config, is_enabled, created_at, updated_at
         FROM providers
         WHERE id = $1",
    )
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
) -> Result<Provider> {
    let row = sqlx::query_as::<_, Provider>(
        "INSERT INTO providers (name, base_url, api_format)
         VALUES ($1, $2, $3)
         RETURNING id, name, base_url, api_format, auth_type, auth_config, is_enabled, created_at, updated_at",
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
) -> Result<Option<Provider>> {
    let row = sqlx::query_as::<_, Provider>(
        "UPDATE providers
         SET name = $2, base_url = $3, api_format = $4, is_enabled = $5, updated_at = now()
         WHERE id = $1
         RETURNING id, name, base_url, api_format, auth_type, auth_config, is_enabled, created_at, updated_at",
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

pub async fn delete_provider(pool: &PgPool, id: Uuid) -> Result<bool> {
    let result = sqlx::query("DELETE FROM providers WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}
