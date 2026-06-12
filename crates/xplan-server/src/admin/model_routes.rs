use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use uuid::Uuid;
use xplan_cache::InvalidationEvent;
use xplan_db::repo_model;

use crate::state::AppState;

use super::models::{
    CreateKeyModelAccessBody, CreateModelBody, CreateProviderModelBody, ListProviderModelsQuery,
    UpdateModelBody, UpdateProviderModelBody,
};

pub async fn list_models(State(state): State<AppState>) -> impl IntoResponse {
    match repo_model::list_models(state.pool()).await {
        Ok(models) => (StatusCode::OK, Json(models)).into_response(),
        Err(e) => {
            tracing::error!("list_models error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

pub async fn create_model(
    State(state): State<AppState>,
    Json(body): Json<CreateModelBody>,
) -> impl IntoResponse {
    match repo_model::create_model(state.pool(), &body.name).await {
        Ok(model) => {
            let _ = state.publisher().publish(&InvalidationEvent::Routes).await;
            (StatusCode::CREATED, Json(model)).into_response()
        }
        Err(e) => {
            tracing::error!("create_model error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

pub async fn delete_model(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match repo_model::delete_model(state.pool(), id).await {
        Ok(true) => {
            let _ = state.publisher().publish(&InvalidationEvent::Routes).await;
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(false) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "model not found"}))).into_response(),
        Err(e) => {
            tracing::error!("delete_model error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

pub async fn update_model(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateModelBody>,
) -> impl IntoResponse {
    match repo_model::update_model(state.pool(), id, &body.name, body.is_enabled).await {
        Ok(Some(model)) => {
            let _ = state.publisher().publish(&InvalidationEvent::Routes).await;
            (StatusCode::OK, Json(serde_json::to_value(model).unwrap())).into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "model not found"}))).into_response(),
        Err(e) => {
            tracing::error!("update_model error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

pub async fn update_provider_model(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateProviderModelBody>,
) -> impl IntoResponse {
    match repo_model::update_provider_model(
        state.pool(),
        id,
        &body.upstream_model_name,
        body.input_price_per_mtok,
        body.output_price_per_mtok,
        body.cache_read_price_per_mtok,
        body.cache_write_price_per_mtok,
        body.config,
    )
    .await
    {
        Ok(Some(pm)) => {
            let _ = state.publisher().publish(&InvalidationEvent::Routes).await;
            let _ = state.publisher().publish(&InvalidationEvent::Prices).await;
            (StatusCode::OK, Json(serde_json::to_value(pm).unwrap())).into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "provider model not found"}))).into_response(),
        Err(e) => {
            tracing::error!("update_provider_model error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

pub async fn list_provider_models(
    State(state): State<AppState>,
    Query(query): Query<ListProviderModelsQuery>,
) -> impl IntoResponse {
    match repo_model::list_provider_models(state.pool(), query.model_id).await {
        Ok(models) => (StatusCode::OK, Json(models)).into_response(),
        Err(e) => {
            tracing::error!("list_provider_models error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

pub async fn create_provider_model(
    State(state): State<AppState>,
    Json(body): Json<CreateProviderModelBody>,
) -> impl IntoResponse {
    match repo_model::create_provider_model(
        state.pool(),
        body.provider_id,
        body.model_id,
        &body.upstream_model_name,
        body.input_price_per_mtok,
        body.output_price_per_mtok,
        body.cache_read_price_per_mtok,
        body.cache_write_price_per_mtok,
        body.config,
    )
    .await
    {
        Ok(pm) => {
            let _ = state.publisher().publish(&InvalidationEvent::Routes).await;
            let _ = state.publisher().publish(&InvalidationEvent::Prices).await;
            (StatusCode::CREATED, Json(pm)).into_response()
        }
        Err(e) => {
            tracing::error!("create_provider_model error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

pub async fn delete_provider_model(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match repo_model::delete_provider_model(state.pool(), id).await {
        Ok(true) => {
            let _ = state.publisher().publish(&InvalidationEvent::Routes).await;
            let _ = state.publisher().publish(&InvalidationEvent::Prices).await;
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn create_key_model_access(
    State(state): State<AppState>,
    Json(body): Json<CreateKeyModelAccessBody>,
) -> impl IntoResponse {
    match repo_model::create_key_model_access(
        state.pool(),
        body.upstream_key_id,
        body.provider_model_id,
        body.priority,
        body.weight,
    )
    .await
    {
        Ok(access) => {
            let _ = state.publisher().publish(&InvalidationEvent::Routes).await;
            (StatusCode::CREATED, Json(access)).into_response()
        }
        Err(e) => {
            tracing::error!("create_key_model_access error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct ListKeyModelAccessParams {
    pub provider_model_id: Option<Uuid>,
}

pub async fn list_key_model_access(
    State(state): State<AppState>,
    Query(params): Query<ListKeyModelAccessParams>,
) -> impl IntoResponse {
    match repo_model::list_key_model_access(state.pool(), params.provider_model_id).await {
        Ok(list) => Json(serde_json::json!(list)).into_response(),
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

pub async fn delete_key_model_access(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match repo_model::delete_key_model_access(state.pool(), id).await {
        Ok(true) => {
            let _ = state.publisher().publish(&InvalidationEvent::Routes).await;
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
