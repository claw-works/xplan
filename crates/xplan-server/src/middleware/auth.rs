use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use xplan_core::{auth::is_admin, AuthError};

use crate::state::AppState;

/// Extract an API key from request headers.
/// Tries `Authorization: Bearer <key>` first, then `x-api-key: <key>`.
fn extract_api_key(headers: &HeaderMap) -> Option<String> {
    // Try Authorization: Bearer <token>
    if let Some(auth_value) = headers.get("Authorization") {
        if let Ok(auth_str) = auth_value.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                let token = token.trim();
                if !token.is_empty() {
                    return Some(token.to_string());
                }
            }
        }
    }

    // Try x-api-key header
    if let Some(key_value) = headers.get("x-api-key") {
        if let Ok(key_str) = key_value.to_str() {
            let key_str = key_str.trim();
            if !key_str.is_empty() {
                return Some(key_str.to_string());
            }
        }
    }

    None
}

/// Middleware that authenticates proxy requests via API key.
///
/// Extracts the API key from `Authorization: Bearer` or `x-api-key` headers,
/// authenticates it via [`AuthService`], and inserts the resolved
/// [`CachedClientKey`] into the request extensions.
pub async fn proxy_auth(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Response {
    let api_key = match extract_api_key(req.headers()) {
        Some(key) => key,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "error": {
                        "message": "Missing API key. Provide it via Authorization: Bearer or x-api-key header.",
                        "type": "authentication_error",
                        "code": "missing_api_key"
                    }
                })),
            )
                .into_response();
        }
    };

    match state.auth().authenticate(&api_key).await {
        Ok(client_key) => {
            req.extensions_mut().insert(client_key);
            next.run(req).await
        }
        Err(AuthError::InvalidKey) => (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "error": {
                    "message": "Invalid API key.",
                    "type": "authentication_error",
                    "code": "invalid_api_key"
                }
            })),
        )
            .into_response(),
        Err(AuthError::Disabled) => (
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": {
                    "message": "API key is disabled.",
                    "type": "authentication_error",
                    "code": "api_key_disabled"
                }
            })),
        )
            .into_response(),
        Err(AuthError::ModelNotAllowed) => (
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": {
                    "message": "Model access not allowed.",
                    "type": "authentication_error",
                    "code": "model_not_allowed"
                }
            })),
        )
            .into_response(),
        Err(AuthError::Internal(msg)) => {
            tracing::error!("Auth internal error: {}", msg);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": {
                        "message": "Internal authentication error.",
                        "type": "internal_error",
                        "code": "internal_error"
                    }
                })),
            )
                .into_response()
        }
    }
}

/// Middleware that authenticates admin requests via a client key with admin role.
///
/// Extracts the API key from `Authorization: Bearer` or `x-api-key` headers,
/// authenticates it via [`AuthService`], and checks that the key has `role = admin`.
pub async fn admin_auth(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Response {
    let api_key = match extract_api_key(req.headers()) {
        Some(key) => key,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "error": {
                        "message": "Missing API key.",
                        "type": "authentication_error",
                        "code": "missing_api_key"
                    }
                })),
            )
                .into_response();
        }
    };

    match state.auth().authenticate(&api_key).await {
        Ok(client_key) => {
            if !is_admin(&client_key) {
                return (
                    StatusCode::FORBIDDEN,
                    Json(json!({
                        "error": {
                            "message": "Admin access required.",
                            "type": "authorization_error",
                            "code": "admin_required"
                        }
                    })),
                )
                    .into_response();
            }
            req.extensions_mut().insert(client_key);
            next.run(req).await
        }
        Err(AuthError::InvalidKey) => (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "error": {
                    "message": "Invalid API key.",
                    "type": "authentication_error",
                    "code": "invalid_api_key"
                }
            })),
        )
            .into_response(),
        Err(AuthError::Disabled) => (
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": {
                    "message": "API key is disabled.",
                    "type": "authentication_error",
                    "code": "api_key_disabled"
                }
            })),
        )
            .into_response(),
        Err(AuthError::ModelNotAllowed) => (
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": {
                    "message": "Model access not allowed.",
                    "type": "authentication_error",
                    "code": "model_not_allowed"
                }
            })),
        )
            .into_response(),
        Err(AuthError::Internal(msg)) => {
            tracing::error!("Auth internal error: {}", msg);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": {
                        "message": "Internal authentication error.",
                        "type": "internal_error",
                        "code": "internal_error"
                    }
                })),
            )
                .into_response()
        }
    }
}
