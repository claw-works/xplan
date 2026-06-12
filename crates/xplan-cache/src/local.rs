use moka::future::Cache;
use std::time::Duration;
use uuid::Uuid;

#[derive(Clone)]
pub struct LocalCache {
    pub client_keys: Cache<String, CachedClientKey>,
    pub route_table: Cache<String, Vec<RouteCandidate>>,
    pub price_table: Cache<Uuid, PriceEntry>,
}

#[derive(Clone, Debug)]
pub struct CachedClientKey {
    pub id: Uuid,
    pub name: String,
    pub access_all_models: bool,
    pub is_enabled: bool,
    pub rate_limit_rpm: Option<i32>,
    pub role: String,
}

#[derive(Clone, Debug)]
pub struct RouteCandidate {
    pub key_model_access_id: Uuid,
    pub upstream_key_id: Uuid,
    pub provider_model_id: Uuid,
    pub provider_id: Uuid,
    pub upstream_model_name: String,
    pub base_url: String,
    pub api_format: String,
    pub priority: i32,
    pub weight: i32,
    pub config: serde_json::Value,
}

#[derive(Clone, Debug)]
pub struct PriceEntry {
    pub input_price_per_mtok: i32,
    pub output_price_per_mtok: i32,
    pub cache_read_price_per_mtok: i32,
    pub cache_write_price_per_mtok: i32,
}

impl LocalCache {
    pub fn new() -> Self {
        Self {
            client_keys: Cache::builder()
                .time_to_live(Duration::from_secs(60))
                .max_capacity(10_000)
                .build(),
            route_table: Cache::builder()
                .time_to_live(Duration::from_secs(300))
                .max_capacity(1_000)
                .build(),
            price_table: Cache::builder()
                .time_to_live(Duration::from_secs(300))
                .max_capacity(1_000)
                .build(),
        }
    }

    pub fn invalidate_all(&self) {
        self.client_keys.invalidate_all();
        self.route_table.invalidate_all();
        self.price_table.invalidate_all();
    }

    pub fn invalidate_routes(&self) {
        self.route_table.invalidate_all();
    }

    pub async fn invalidate_client_key(&self, key_hash: &str) {
        self.client_keys.invalidate(key_hash).await;
    }
}

impl Default for LocalCache {
    fn default() -> Self {
        Self::new()
    }
}
