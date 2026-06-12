pub mod chat;
pub mod messages;
pub mod models;

use axum::{middleware, routing::{get, post}, Router};
use crate::state::AppState;
use crate::middleware::auth::proxy_auth;

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/v1/chat/completions", post(chat::handle_chat_completion))
        .route("/v1/messages", post(messages::handle_messages))
        .route("/v1/models", get(models::handle_list_models))
        .route_layer(middleware::from_fn_with_state(state, proxy_auth))
}
