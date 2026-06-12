use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use xplan_db::repo_usage;

use crate::state::AppState;

use super::models::{UsageQuery, UsageSummaryQuery, UsageBreakdownQuery};

pub async fn query(
    State(state): State<AppState>,
    Query(params): Query<UsageQuery>,
) -> impl IntoResponse {
    let now = Utc::now();
    let from = params.from.unwrap_or_else(|| now - chrono::Duration::days(7));
    let to = params.to.unwrap_or(now);
    let limit = params.limit.unwrap_or(100).min(1000);
    let offset = params.offset.unwrap_or(0);

    match repo_usage::query_usage(
        state.pool(),
        from,
        to,
        params.model.as_deref(),
        params.provider.as_deref(),
        limit,
        offset,
    )
    .await
    {
        Ok(logs) => (StatusCode::OK, Json(logs)).into_response(),
        Err(e) => {
            tracing::error!("query_usage error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

pub async fn summary(
    State(state): State<AppState>,
    Query(params): Query<UsageSummaryQuery>,
) -> impl IntoResponse {
    let now = Utc::now();
    let from = params.from.unwrap_or_else(|| now - chrono::Duration::days(7));
    let to = params.to.unwrap_or(now);

    match repo_usage::usage_summary(state.pool(), from, to).await {
        Ok(rows) => (StatusCode::OK, Json(rows)).into_response(),
        Err(e) => {
            tracing::error!("usage_summary error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

pub async fn by_upstream_key(
    State(state): State<AppState>,
    Query(params): Query<UsageBreakdownQuery>,
) -> impl IntoResponse {
    let now = Utc::now();
    let from = params.from.unwrap_or_else(|| now - chrono::Duration::days(7));
    let to = params.to.unwrap_or(now);

    match repo_usage::usage_by_upstream_key(state.pool(), from, to).await {
        Ok(rows) => (StatusCode::OK, Json(rows)).into_response(),
        Err(e) => {
            tracing::error!("usage_by_upstream_key error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

pub async fn by_client_key(
    State(state): State<AppState>,
    Query(params): Query<UsageBreakdownQuery>,
) -> impl IntoResponse {
    let now = Utc::now();
    let from = params.from.unwrap_or_else(|| now - chrono::Duration::days(7));
    let to = params.to.unwrap_or(now);

    match repo_usage::usage_by_client_key(state.pool(), from, to).await {
        Ok(rows) => (StatusCode::OK, Json(rows)).into_response(),
        Err(e) => {
            tracing::error!("usage_by_client_key error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}
