use sha2::{Digest, Sha256};
use uuid::Uuid;
use xplan_cache::{CachedClientKey, LocalCache};
use xplan_db::models::ClientRole;
use xplan_db::PgPool;

pub struct AuthService {
    pool: PgPool,
    cache: LocalCache,
}

impl AuthService {
    pub fn new(pool: PgPool, cache: LocalCache) -> Self {
        Self { pool, cache }
    }

    pub async fn authenticate(&self, raw_key: &str) -> Result<CachedClientKey, AuthError> {
        let hashed = hash_key(raw_key);

        // Check local cache first
        if let Some(cached) = self.cache.client_keys.get(&hashed).await {
            if !cached.is_enabled {
                return Err(AuthError::Disabled);
            }
            return Ok(cached);
        }

        // Cache miss — query DB
        let row = sqlx::query_as::<_, ClientKeyRow>(
            "SELECT id, name, access_all_models, is_enabled, rate_limit_rpm, role \
             FROM client_keys WHERE key_hash = $1",
        )
        .bind(&hashed)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AuthError::Internal(e.to_string()))?;

        let row = row.ok_or(AuthError::InvalidKey)?;

        let key = CachedClientKey {
            id: row.id,
            name: row.name,
            access_all_models: row.access_all_models,
            is_enabled: row.is_enabled,
            rate_limit_rpm: row.rate_limit_rpm,
            role: match row.role {
                ClientRole::Admin => "admin".to_string(),
                ClientRole::User => "user".to_string(),
            },
        };

        // Store in cache regardless of is_enabled so we don't spam DB
        self.cache.client_keys.insert(hashed, key.clone()).await;

        if !key.is_enabled {
            return Err(AuthError::Disabled);
        }

        Ok(key)
    }

    pub async fn check_model_access(
        &self,
        client_key_id: Uuid,
        model_name: &str,
        access_all: bool,
    ) -> Result<(), AuthError> {
        if access_all {
            return Ok(());
        }

        let row: (bool,) = sqlx::query_as::<_, (bool,)>(
            "SELECT EXISTS(\
               SELECT 1 FROM client_model_access cma \
               JOIN models m ON m.id = cma.model_id \
               WHERE cma.client_key_id = $1 AND m.name = $2 AND cma.is_enabled = true\
             )",
        )
        .bind(client_key_id)
        .bind(model_name)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AuthError::Internal(e.to_string()))?;

        if row.0 {
            Ok(())
        } else {
            Err(AuthError::ModelNotAllowed)
        }
    }
}

/// Returns true if the cached client key has the admin role.
pub fn is_admin(client: &CachedClientKey) -> bool {
    client.role == "admin"
}

/// SHA-256 hex digest of a raw API key.
pub fn hash_key(raw_key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw_key.as_bytes());
    hex::encode(hasher.finalize())
}

// Internal row type for sqlx
#[derive(sqlx::FromRow)]
struct ClientKeyRow {
    id: Uuid,
    name: String,
    access_all_models: bool,
    is_enabled: bool,
    rate_limit_rpm: Option<i32>,
    role: ClientRole,
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("invalid API key")]
    InvalidKey,
    #[error("API key is disabled")]
    Disabled,
    #[error("model access not allowed")]
    ModelNotAllowed,
    #[error("internal error: {0}")]
    Internal(String),
}
