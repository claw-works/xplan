use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use uuid::Uuid;
use xplan_cache::InvalidationEvent;
use xplan_core::auth::hash_key;
use xplan_db::{models::ClientRole, repo_client_key};

use crate::state::AppState;

use super::models::{CreateClientKeyBody, CreateClientKeyResponse};

fn generate_random_key() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut bytes);
    format!("sk-xplan-{}", hex::encode(bytes))
}

pub async fn list(State(state): State<AppState>) -> impl IntoResponse {
    match repo_client_key::list_client_keys(state.pool()).await {
        Ok(keys) => {
            // Strip key_hash from response — build a safe view
            let safe: Vec<serde_json::Value> = keys
                .into_iter()
                .map(|k| {
                    serde_json::json!({
                        "id": k.id,
                        "name": k.name,
                        "key_prefix": k.key_prefix,
                        "access_all_models": k.access_all_models,
                        "is_enabled": k.is_enabled,
                        "rate_limit_rpm": k.rate_limit_rpm,
                        "role": k.role,
                        "created_at": k.created_at,
                        "updated_at": k.updated_at,
                    })
                })
                .collect();
            (StatusCode::OK, Json(safe)).into_response()
        }
        Err(e) => {
            tracing::error!("list_client_keys error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

pub async fn create(
    State(state): State<AppState>,
    Json(body): Json<CreateClientKeyBody>,
) -> impl IntoResponse {
    let raw_key = generate_random_key();
    let hashed = hash_key(&raw_key);
    // prefix is "sk-xplan-" + first 8 hex chars of body
    let key_prefix = raw_key[..17].to_string(); // "sk-xplan-" is 9, + 8 chars = 17

    let access_all = body.access_all_models.unwrap_or(true);
    let role = match body.role.as_deref() {
        Some("admin") => ClientRole::Admin,
        _ => ClientRole::User,
    };

    match repo_client_key::create_client_key(
        state.pool(),
        &body.name,
        &hashed,
        &key_prefix,
        access_all,
        body.rate_limit_rpm,
        role,
    )
    .await
    {
        Ok(k) => {
            let resp = CreateClientKeyResponse {
                id: k.id,
                name: k.name,
                key: raw_key,
                key_prefix: k.key_prefix,
                access_all_models: k.access_all_models,
                is_enabled: k.is_enabled,
                rate_limit_rpm: k.rate_limit_rpm,
                created_at: k.created_at,
                updated_at: k.updated_at,
            };
            let _ = state.publisher().publish(&InvalidationEvent::All).await;
            (StatusCode::CREATED, Json(resp)).into_response()
        }
        Err(e) => {
            tracing::error!("create_client_key error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

pub async fn remove(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match repo_client_key::delete_client_key(state.pool(), id).await {
        Ok(true) => {
            let _ = state.publisher().publish(&InvalidationEvent::All).await;
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(false) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "client key not found"}))).into_response(),
        Err(e) => {
            tracing::error!("delete_client_key error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}
