use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use uuid::Uuid;
use xplan_cache::InvalidationEvent;
use xplan_db::repo_provider;

use crate::state::AppState;

use super::models::{CreateProviderBody, UpdateProviderBody};

pub async fn list(State(state): State<AppState>) -> impl IntoResponse {
    match repo_provider::list_providers(state.pool()).await {
        Ok(providers) => (StatusCode::OK, Json(providers)).into_response(),
        Err(e) => {
            tracing::error!("list_providers error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

pub async fn create(
    State(state): State<AppState>,
    Json(body): Json<CreateProviderBody>,
) -> impl IntoResponse {
    match repo_provider::create_provider(state.pool(), &body.name, &body.base_url, body.api_format).await {
        Ok(provider) => {
            let _ = state.publisher().publish(&InvalidationEvent::Routes).await;
            (StatusCode::CREATED, Json(provider)).into_response()
        }
        Err(e) => {
            tracing::error!("create_provider error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateProviderBody>,
) -> impl IntoResponse {
    match repo_provider::update_provider(
        state.pool(),
        id,
        &body.name,
        &body.base_url,
        body.api_format,
        body.is_enabled,
    )
    .await
    {
        Ok(Some(provider)) => {
            let _ = state.publisher().publish(&InvalidationEvent::Routes).await;
            (StatusCode::OK, Json(provider)).into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "provider not found"}))).into_response(),
        Err(e) => {
            tracing::error!("update_provider error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

pub async fn remove(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match repo_provider::delete_provider(state.pool(), id).await {
        Ok(true) => {
            let _ = state.publisher().publish(&InvalidationEvent::Routes).await;
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(false) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "provider not found"}))).into_response(),
        Err(e) => {
            tracing::error!("delete_provider error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}
