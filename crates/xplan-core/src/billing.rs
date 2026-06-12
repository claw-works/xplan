use uuid::Uuid;
use xplan_cache::{LocalCache, PriceEntry};
use xplan_db::PgPool;
use xplan_provider::TokenUsage;

pub struct BillingEngine {
    pool: PgPool,
    cache: LocalCache,
}

impl BillingEngine {
    pub fn new(pool: PgPool, cache: LocalCache) -> Self {
        Self { pool, cache }
    }

    pub async fn calculate_cost(
        &self,
        provider_model_id: Uuid,
        usage: &TokenUsage,
    ) -> anyhow::Result<i64> {
        let price = self.get_price(provider_model_id).await?;

        // Store cost in micro-cents (price_per_mtok * tokens = cents * 10^-6).
        // Omitting the /1_000_000 division preserves precision for small requests.
        let cost: i64 = (usage.input_tokens as i64 * price.input_price_per_mtok as i64)
            + (usage.output_tokens as i64 * price.output_price_per_mtok as i64)
            + (usage.cache_read_tokens as i64 * price.cache_read_price_per_mtok as i64)
            + (usage.cache_write_tokens as i64 * price.cache_write_price_per_mtok as i64);

        Ok(cost)
    }

    async fn get_price(&self, provider_model_id: Uuid) -> anyhow::Result<PriceEntry> {
        // Check local cache first
        if let Some(cached) = self.cache.price_table.get(&provider_model_id).await {
            return Ok(cached);
        }

        // Cache miss — query DB
        let row = sqlx::query_as::<_, PriceRow>(
            "SELECT input_price_per_mtok, output_price_per_mtok, \
               cache_read_price_per_mtok, cache_write_price_per_mtok \
             FROM provider_models WHERE id = $1",
        )
        .bind(provider_model_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("price not found for provider_model_id: {}", provider_model_id))?;

        let entry = PriceEntry {
            input_price_per_mtok: row.input_price_per_mtok,
            output_price_per_mtok: row.output_price_per_mtok,
            cache_read_price_per_mtok: row.cache_read_price_per_mtok,
            cache_write_price_per_mtok: row.cache_write_price_per_mtok,
        };

        // Store in cache
        self.cache
            .price_table
            .insert(provider_model_id, entry.clone())
            .await;

        Ok(entry)
    }
}

// Internal row type for sqlx
#[derive(sqlx::FromRow)]
struct PriceRow {
    input_price_per_mtok: i32,
    output_price_per_mtok: i32,
    cache_read_price_per_mtok: i32,
    cache_write_price_per_mtok: i32,
}
