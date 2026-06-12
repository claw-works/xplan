mod admin;
mod config;
mod middleware;
mod proxy;
mod state;

use axum::routing::get;
use config::AppConfig;
use tower_http::services::ServeDir;
use state::{AppState, AppStateInner};
use tokio::sync::broadcast;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use xplan_cache::{
    spawn_invalidation_listener, InvalidationEvent, InvalidationPublisher, LocalCache,
    QuotaTracker, create_redis_pool,
};
use xplan_core::{AuthService, BillingEngine, QualityMonitor, RouterEngine};
use xplan_db::create_pool;
use xplan_provider::{AnthropicAdapter, BedrockAdapter, OpenAiAdapter, ResponsesAdapter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // Handle CLI subcommands
    if args.len() > 1 && args[1] == "create-admin-key" {
        return create_admin_key_cmd().await;
    }

    // 1. Init tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "xplan=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // 2. Load config
    let config = AppConfig::load()?;
    tracing::info!(
        "Starting xplan on {}:{}",
        config.server.host,
        config.server.port
    );

    // 3. Create PG pool
    let pool = create_pool(&config.database.url, config.database.max_connections).await?;
    tracing::info!("Connected to PostgreSQL");

    // 3b. Run migrations
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await?;
    tracing::info!("Migrations applied");

    // 3c. Bootstrap: create admin key if none exists
    let admin_count = xplan_db::repo_client_key::count_admin_keys(&pool).await?;
    if admin_count == 0 {
        let raw_key = {
            use rand::RngCore;
            let mut bytes = [0u8; 24];
            rand::thread_rng().fill_bytes(&mut bytes);
            format!("sk-xplan-{}", hex::encode(bytes))
        };
        let key_hash = xplan_core::auth::hash_key(&raw_key);
        let key_prefix = raw_key[..17].to_string(); // "sk-xplan-" (9) + 8 hex chars = 17
        xplan_db::repo_client_key::create_client_key(
            &pool,
            "System Admin",
            &key_hash,
            &key_prefix,
            true,  // access_all_models
            None,  // no rate limit
            xplan_db::models::ClientRole::Admin,
        )
        .await?;
        tracing::warn!("===========================================");
        tracing::warn!("  No admin key found. Created initial admin key:");
        tracing::warn!("  {}", raw_key);
        tracing::warn!("  Save this key! It won't be shown again.");
        tracing::warn!("===========================================");
    }

    // 4. Create Redis pool
    let redis_pool = create_redis_pool(&config.redis.url)?;
    tracing::info!("Connected to Redis");

    // 5. Create LocalCache, QuotaTracker, InvalidationPublisher
    let local_cache = LocalCache::new();
    let quota_tracker = QuotaTracker::new(redis_pool.clone());
    let invalidation_publisher = InvalidationPublisher::new(redis_pool.clone());

    // 6. Spawn invalidation listener + local cache invalidation handler
    let (invalidation_tx, _invalidation_rx) = broadcast::channel::<InvalidationEvent>(128);
    spawn_invalidation_listener(config.redis.url.clone(), invalidation_tx.clone());

    // Subscribe to invalidation events and apply them to the local cache
    let cache_for_listener = local_cache.clone();
    let mut rx = invalidation_tx.subscribe();
    tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            match event {
                InvalidationEvent::Routes => {
                    cache_for_listener.invalidate_routes();
                    tracing::debug!("Cache: invalidated routes");
                }
                InvalidationEvent::ClientKey(hash) => {
                    cache_for_listener.invalidate_client_key(&hash).await;
                    tracing::debug!("Cache: invalidated client key {}", hash);
                }
                InvalidationEvent::Prices => {
                    cache_for_listener.price_table.invalidate_all();
                    tracing::debug!("Cache: invalidated prices");
                }
                InvalidationEvent::All => {
                    cache_for_listener.invalidate_all();
                    tracing::debug!("Cache: invalidated all");
                }
            }
        }
    });

    // 7. Create core services
    let auth_service = AuthService::new(pool.clone(), local_cache.clone());
    let router_engine = RouterEngine::new(pool.clone(), local_cache.clone(), quota_tracker);
    let billing_engine = BillingEngine::new(pool.clone(), local_cache.clone());
    let quality_monitor = QualityMonitor::new();

    // 8. Decode encryption key from hex
    let encryption_key = hex::decode(&config.encryption.key_hex).map_err(|e| {
        anyhow::anyhow!("Failed to decode encryption key from hex: {}", e)
    })?;

    // 9. Build AppState
    let state = AppState::new(AppStateInner {
        pool,
        auth: auth_service,
        router: router_engine,
        billing: billing_engine,
        quality: quality_monitor,
        cache: local_cache,
        invalidation_publisher,
        openai_adapter: OpenAiAdapter::new(),
        anthropic_adapter: AnthropicAdapter::new(),
        bedrock_adapter: BedrockAdapter::new(),
        responses_adapter: ResponsesAdapter::new(),
        encryption_key,
    });

    // 10. Build Router
    let app = axum::Router::new()
        .route("/health", get(health_handler))
        .merge(proxy::router(state.clone()))
        .merge(admin::router(state.clone()))
        // Serve the built frontend from frontend/dist at /admin/
        .nest_service("/admin", ServeDir::new("frontend/dist").append_index_html_on_directories(true))
        .with_state(state);

    // 11. Serve
    let bind_addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    tracing::info!("Listening on {}", listener.local_addr()?);
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_handler() -> &'static str {
    "ok"
}

async fn create_admin_key_cmd() -> anyhow::Result<()> {
    let config = AppConfig::load()?;
    let pool = create_pool(&config.database.url, config.database.max_connections).await?;

    // Run migrations first
    sqlx::migrate!("../../migrations").run(&pool).await?;

    // Generate new admin key
    let raw_key = {
        use rand::RngCore;
        let mut bytes = [0u8; 24];
        rand::thread_rng().fill_bytes(&mut bytes);
        format!("sk-xplan-{}", hex::encode(bytes))
    };
    let key_hash = xplan_core::auth::hash_key(&raw_key);
    let key_prefix = raw_key[..17].to_string();

    xplan_db::repo_client_key::create_client_key(
        &pool,
        "Admin (CLI)",
        &key_hash,
        &key_prefix,
        true,
        None,
        xplan_db::models::ClientRole::Admin,
    )
    .await?;

    println!("=========================================");
    println!("  New admin key created:");
    println!("  {}", raw_key);
    println!("  Save this key! It won't be shown again.");
    println!("=========================================");

    Ok(())
}
