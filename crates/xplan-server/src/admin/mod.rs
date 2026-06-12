pub mod client_keys;
pub mod dashboard;
pub mod model_routes;
pub mod models;
pub mod providers;
pub mod upstream_keys;
pub mod usage;

use axum::{
    middleware,
    routing::{delete, get, post, put},
    Router,
};

use crate::middleware::auth::admin_auth;
use crate::state::AppState;

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        // Providers
        .route("/admin/api/providers", get(providers::list).post(providers::create))
        .route(
            "/admin/api/providers/{id}",
            put(providers::update).delete(providers::remove),
        )
        // Upstream keys
        .route(
            "/admin/api/upstream-keys",
            get(upstream_keys::list).post(upstream_keys::create),
        )
        .route("/admin/api/upstream-keys/{id}", delete(upstream_keys::remove))
        .route(
            "/admin/api/upstream-keys/{id}/models",
            get(upstream_keys::list_upstream_models),
        )
        // Models
        .route(
            "/admin/api/models",
            get(model_routes::list_models).post(model_routes::create_model),
        )
        .route(
            "/admin/api/models/{id}",
            put(model_routes::update_model).delete(model_routes::delete_model),
        )
        // Provider models
        .route(
            "/admin/api/provider-models",
            get(model_routes::list_provider_models).post(model_routes::create_provider_model),
        )
        .route(
            "/admin/api/provider-models/{id}",
            put(model_routes::update_provider_model).delete(model_routes::delete_provider_model),
        )
        // Key model access
        .route(
            "/admin/api/key-model-access",
            get(model_routes::list_key_model_access).post(model_routes::create_key_model_access),
        )
        .route(
            "/admin/api/key-model-access/{id}",
            delete(model_routes::delete_key_model_access),
        )
        // Client keys
        .route(
            "/admin/api/client-keys",
            get(client_keys::list).post(client_keys::create),
        )
        .route("/admin/api/client-keys/{id}", delete(client_keys::remove))
        // Usage
        .route("/admin/api/usage", get(usage::query))
        .route("/admin/api/usage/summary", get(usage::summary))
        .route("/admin/api/usage/by-upstream-key", get(usage::by_upstream_key))
        .route("/admin/api/usage/by-client-key", get(usage::by_client_key))
        // Dashboard
        .route("/admin/api/dashboard", get(dashboard::overview))
        // Apply admin auth middleware to all routes
        .route_layer(middleware::from_fn_with_state(state, admin_auth))
}
