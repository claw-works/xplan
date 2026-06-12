use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use uuid::Uuid;
use xplan_cache::InvalidationEvent;
use xplan_db::{models::ApiFormat, repo_provider, repo_upstream_key};

use crate::state::AppState;

use super::models::{CreateUpstreamKeyBody, ListUpstreamKeysQuery, UpstreamKeyResponse};

fn encrypt_key(plaintext: &str, key: &[u8]) -> anyhow::Result<Vec<u8>> {
    use aes_gcm::{aead::Aead, Aes256Gcm, KeyInit, Nonce};
    use rand::RngCore;

    let cipher = Aes256Gcm::new_from_slice(key)?;
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| anyhow::anyhow!("encrypt failed: {}", e))?;
    let mut result = nonce_bytes.to_vec();
    result.extend(ciphertext);
    Ok(result)
}

fn decrypt_key(ciphertext: &[u8], key: &[u8]) -> anyhow::Result<String> {
    use aes_gcm::{aead::Aead, Aes256Gcm, KeyInit, Nonce};

    if ciphertext.len() < 12 {
        anyhow::bail!("ciphertext too short");
    }
    let (nonce_bytes, ct) = ciphertext.split_at(12);
    let cipher = Aes256Gcm::new_from_slice(key)?;
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ct)
        .map_err(|e| anyhow::anyhow!("decrypt failed: {}", e))?;
    Ok(String::from_utf8(plaintext)?)
}

fn to_response(k: xplan_db::models::UpstreamKey) -> UpstreamKeyResponse {
    UpstreamKeyResponse {
        id: k.id,
        provider_id: k.provider_id,
        alias: k.alias,
        is_enabled: k.is_enabled,
        status: k.status,
        created_at: k.created_at,
        updated_at: k.updated_at,
    }
}

pub async fn list(
    State(state): State<AppState>,
    Query(query): Query<ListUpstreamKeysQuery>,
) -> impl IntoResponse {
    match repo_upstream_key::list_upstream_keys(state.pool(), query.provider_id).await {
        Ok(keys) => {
            let resp: Vec<UpstreamKeyResponse> = keys.into_iter().map(to_response).collect();
            (StatusCode::OK, Json(resp)).into_response()
        }
        Err(e) => {
            tracing::error!("list_upstream_keys error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

pub async fn create(
    State(state): State<AppState>,
    Json(body): Json<CreateUpstreamKeyBody>,
) -> impl IntoResponse {
    let encrypted = match encrypt_key(&body.api_key, state.encryption_key()) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("encrypt_key error: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "encryption failed"}))).into_response();
        }
    };

    match repo_upstream_key::create_upstream_key(state.pool(), body.provider_id, &body.alias, &encrypted).await {
        Ok(key) => {
            let _ = state.publisher().publish(&InvalidationEvent::Routes).await;
            (StatusCode::CREATED, Json(to_response(key))).into_response()
        }
        Err(e) => {
            tracing::error!("create_upstream_key error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

pub async fn remove(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match repo_upstream_key::delete_upstream_key(state.pool(), id).await {
        Ok(true) => {
            let _ = state.publisher().publish(&InvalidationEvent::Routes).await;
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(false) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "key not found"}))).into_response(),
        Err(e) => {
            tracing::error!("delete_upstream_key error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

pub async fn list_upstream_models(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    // 1. Get upstream key
    let key_row = match repo_upstream_key::get_upstream_key(state.pool(), id).await {
        Ok(Some(k)) => k,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"models": [], "error": "upstream key not found"})),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("get_upstream_key error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"models": [], "error": e.to_string()})),
            )
                .into_response();
        }
    };

    // 2. Get provider
    let provider = match repo_provider::get_provider(state.pool(), key_row.provider_id).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"models": [], "error": "provider not found"})),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("get_provider error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"models": [], "error": e.to_string()})),
            )
                .into_response();
        }
    };

    // 3. Check api_format
    if provider.api_format != ApiFormat::OpenaiCompatible {
        return Json(json!({
            "models": [],
            "error": "List models not supported for this provider format"
        }))
        .into_response();
    }

    // 4. Decrypt API key
    let api_key = match decrypt_key(&key_row.api_key_encrypted, state.encryption_key()) {
        Ok(k) => k,
        Err(e) => {
            tracing::error!("decrypt_key error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"models": [], "error": "failed to decrypt key"})),
            )
                .into_response();
        }
    };

    // 5. Call upstream GET {base_url}/models
    let url = format!("{}/models", provider.base_url.trim_end_matches('/'));
    let client = reqwest::Client::new();
    let resp = match client.get(&url).bearer_auth(&api_key).send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("upstream models request error: {}", e);
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({"models": [], "error": format!("upstream request failed: {}", e)})),
            )
                .into_response();
        }
    };

    let status_code = resp.status();
    let raw_text = match resp.text().await {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("upstream models response read error: {}", e);
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({"models": [], "error": format!("failed to read upstream response: {}", e)})),
            )
                .into_response();
        }
    };

    if !status_code.is_success() {
        tracing::warn!("upstream models returned {} for URL {}: {}", status_code, url, &raw_text[..raw_text.len().min(200)]);
        return (
            StatusCode::BAD_GATEWAY,
            Json(json!({"models": [], "error": format!("upstream returned HTTP {} for {}: {}", status_code, url, &raw_text[..raw_text.len().min(200)])})),
        )
            .into_response();
    }

    let body: serde_json::Value = match serde_json::from_str(&raw_text) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("upstream models JSON parse error: {} — body: {}", e, &raw_text[..raw_text.len().min(200)]);
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({"models": [], "error": format!("failed to parse JSON: {} — raw: {}", e, &raw_text[..raw_text.len().min(100)])})),
            )
                .into_response();
        }
    };

    // 6. Extract model IDs from the "data" array
    let models: Vec<String> = body["data"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|m| m["id"].as_str().map(|s| s.to_string()))
        .collect();

    Json(json!({"models": models})).into_response()
}
