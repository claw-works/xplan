use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use chrono::Utc;
use serde::Serialize;
use xplan_db::repo_usage::{usage_summary, UsageSummary};

use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct DashboardOverview {
    pub from: chrono::DateTime<Utc>,
    pub to: chrono::DateTime<Utc>,
    pub total_requests: i64,
    pub total_cost_cents: i64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub by_model: Vec<UsageSummary>,
}

pub async fn overview(State(state): State<AppState>) -> impl IntoResponse {
    let now = Utc::now();
    let from = now - chrono::Duration::hours(24);

    match usage_summary(state.pool(), from, now).await {
        Ok(rows) => {
            let total_requests: i64 = rows.iter().map(|r| r.total_requests).sum();
            let total_cost_cents: i64 = rows.iter().map(|r| r.total_cost_cents).sum();
            let total_input_tokens: i64 = rows.iter().map(|r| r.total_input_tokens).sum();
            let total_output_tokens: i64 = rows.iter().map(|r| r.total_output_tokens).sum();

            let overview = DashboardOverview {
                from,
                to: now,
                total_requests,
                total_cost_cents,
                total_input_tokens,
                total_output_tokens,
                by_model: rows,
            };

            (StatusCode::OK, Json(overview)).into_response()
        }
        Err(e) => {
            tracing::error!("dashboard overview error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}
