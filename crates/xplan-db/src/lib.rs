pub mod models;
pub mod repo_client_key;
pub mod repo_model;
pub mod repo_provider;
pub mod repo_quality;
pub mod repo_upstream_key;
pub mod repo_usage;

pub use sqlx::PgPool;

pub async fn create_pool(database_url: &str, max_connections: u32) -> anyhow::Result<PgPool> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(max_connections)
        .connect(database_url)
        .await?;
    Ok(pool)
}
