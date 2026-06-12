use axum::{extract::State, Json};
use serde_json::{json, Value};

use crate::state::AppState;

pub async fn handle_list_models(State(state): State<AppState>) -> Json<Value> {
    let models = xplan_db::repo_model::list_models(state.pool())
        .await
        .unwrap_or_default();

    let data: Vec<Value> = models
        .into_iter()
        .filter(|m| m.is_enabled)
        .map(|m| {
            json!({
                "id": m.name,
                "object": "model",
                "created": m.created_at.timestamp(),
                "owned_by": "xplan"
            })
        })
        .collect();

    Json(json!({
        "object": "list",
        "data": data
    }))
}
