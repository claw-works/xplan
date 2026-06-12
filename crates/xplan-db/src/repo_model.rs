use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::{KeyModelAccess, Model, ProviderModel};

pub async fn list_models(pool: &PgPool) -> Result<Vec<Model>> {
    let rows = sqlx::query_as::<_, Model>(
        "SELECT id, name, is_enabled, created_at, updated_at
         FROM models
         ORDER BY name",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn create_model(pool: &PgPool, name: &str) -> Result<Model> {
    let row = sqlx::query_as::<_, Model>(
        "INSERT INTO models (name)
         VALUES ($1)
         RETURNING id, name, is_enabled, created_at, updated_at",
    )
    .bind(name)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn delete_model(pool: &PgPool, id: Uuid) -> Result<bool> {
    let result = sqlx::query("DELETE FROM models WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn list_provider_models(
    pool: &PgPool,
    model_id: Option<Uuid>,
) -> Result<Vec<ProviderModel>> {
    let rows = match model_id {
        Some(mid) => sqlx::query_as::<_, ProviderModel>(
            "SELECT id, provider_id, model_id, upstream_model_name,
                    input_price_per_mtok, output_price_per_mtok,
                    cache_read_price_per_mtok, cache_write_price_per_mtok,
                    config,
                    created_at, updated_at
             FROM provider_models
             WHERE model_id = $1
             ORDER BY created_at",
        )
        .bind(mid)
        .fetch_all(pool)
        .await?,
        None => sqlx::query_as::<_, ProviderModel>(
            "SELECT id, provider_id, model_id, upstream_model_name,
                    input_price_per_mtok, output_price_per_mtok,
                    cache_read_price_per_mtok, cache_write_price_per_mtok,
                    config,
                    created_at, updated_at
             FROM provider_models
             ORDER BY created_at",
        )
        .fetch_all(pool)
        .await?,
    };
    Ok(rows)
}

pub async fn create_provider_model(
    pool: &PgPool,
    provider_id: Uuid,
    model_id: Uuid,
    upstream_model_name: &str,
    input_price: i32,
    output_price: i32,
    cache_read_price: i32,
    cache_write_price: i32,
    config: serde_json::Value,
) -> Result<ProviderModel> {
    let row = sqlx::query_as::<_, ProviderModel>(
        "INSERT INTO provider_models
             (provider_id, model_id, upstream_model_name,
              input_price_per_mtok, output_price_per_mtok,
              cache_read_price_per_mtok, cache_write_price_per_mtok,
              config)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING id, provider_id, model_id, upstream_model_name,
                   input_price_per_mtok, output_price_per_mtok,
                   cache_read_price_per_mtok, cache_write_price_per_mtok,
                   config,
                   created_at, updated_at",
    )
    .bind(provider_id)
    .bind(model_id)
    .bind(upstream_model_name)
    .bind(input_price)
    .bind(output_price)
    .bind(cache_read_price)
    .bind(cache_write_price)
    .bind(config)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn update_model(pool: &PgPool, id: Uuid, name: &str, is_enabled: bool) -> Result<Option<Model>> {
    let row = sqlx::query_as::<_, Model>(
        "UPDATE models SET name = $2, is_enabled = $3 WHERE id = $1 RETURNING id, name, is_enabled, created_at, updated_at"
    )
    .bind(id)
    .bind(name)
    .bind(is_enabled)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn update_provider_model(
    pool: &PgPool,
    id: Uuid,
    upstream_model_name: &str,
    input_price: i32,
    output_price: i32,
    cache_read_price: i32,
    cache_write_price: i32,
    config: serde_json::Value,
) -> Result<Option<ProviderModel>> {
    let row = sqlx::query_as::<_, ProviderModel>(
        "UPDATE provider_models SET upstream_model_name=$2, input_price_per_mtok=$3, output_price_per_mtok=$4, cache_read_price_per_mtok=$5, cache_write_price_per_mtok=$6, config=$7 WHERE id=$1 RETURNING id, provider_id, model_id, upstream_model_name, input_price_per_mtok, output_price_per_mtok, cache_read_price_per_mtok, cache_write_price_per_mtok, config, created_at, updated_at"
    )
    .bind(id)
    .bind(upstream_model_name)
    .bind(input_price)
    .bind(output_price)
    .bind(cache_read_price)
    .bind(cache_write_price)
    .bind(config)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn delete_provider_model(pool: &PgPool, id: Uuid) -> Result<bool> {
    let result = sqlx::query("DELETE FROM provider_models WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn create_key_model_access(
    pool: &PgPool,
    upstream_key_id: Uuid,
    provider_model_id: Uuid,
    priority: i32,
    weight: i32,
) -> Result<KeyModelAccess> {
    let row = sqlx::query_as::<_, KeyModelAccess>(
        "INSERT INTO key_model_access (upstream_key_id, provider_model_id, priority, weight)
         VALUES ($1, $2, $3, $4)
         RETURNING id, upstream_key_id, provider_model_id, priority, weight, is_enabled, created_at, updated_at",
    )
    .bind(upstream_key_id)
    .bind(provider_model_id)
    .bind(priority)
    .bind(weight)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn list_key_model_access(
    pool: &PgPool,
    provider_model_id: Option<Uuid>,
) -> Result<Vec<KeyModelAccess>> {
    if let Some(pm_id) = provider_model_id {
        let rows = sqlx::query_as::<_, KeyModelAccess>(
            "SELECT id, upstream_key_id, provider_model_id, priority, weight, is_enabled, created_at, updated_at \
             FROM key_model_access WHERE provider_model_id = $1 ORDER BY priority ASC, weight DESC",
        )
        .bind(pm_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    } else {
        let rows = sqlx::query_as::<_, KeyModelAccess>(
            "SELECT id, upstream_key_id, provider_model_id, priority, weight, is_enabled, created_at, updated_at \
             FROM key_model_access ORDER BY priority ASC, weight DESC",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }
}

pub async fn delete_key_model_access(pool: &PgPool, id: Uuid) -> Result<bool> {
    let result = sqlx::query("DELETE FROM key_model_access WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}
