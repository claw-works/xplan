use rand::Rng;
use uuid::Uuid;
use xplan_cache::{LocalCache, QuotaTracker, RouteCandidate};
use xplan_db::PgPool;

pub struct RouterEngine {
    pool: PgPool,
    cache: LocalCache,
    quota_tracker: QuotaTracker,
}

impl RouterEngine {
    pub fn new(pool: PgPool, cache: LocalCache, quota_tracker: QuotaTracker) -> Self {
        Self {
            pool,
            cache,
            quota_tracker,
        }
    }

    pub async fn select_upstream(
        &self,
        model_name: &str,
    ) -> Result<SelectedUpstream, RouterError> {
        let candidates = self.get_candidates(model_name).await?;

        if candidates.is_empty() {
            return Err(RouterError::NoRouteFound(model_name.to_string()));
        }

        // Group by priority (ascending) — candidates already ordered by priority ASC
        let mut priority_groups: Vec<Vec<&RouteCandidate>> = Vec::new();
        let mut current_priority: Option<i32> = None;

        for candidate in &candidates {
            if current_priority != Some(candidate.priority) {
                current_priority = Some(candidate.priority);
                priority_groups.push(Vec::new());
            }
            if let Some(group) = priority_groups.last_mut() {
                group.push(candidate);
            }
        }

        // Try each priority group in order
        for group in &priority_groups {
            // Filter candidates whose quota isn't exhausted (check without incrementing)
            let mut available: Vec<&RouteCandidate> = Vec::new();

            for candidate in group.iter() {
                let is_available = self
                    .check_quota_available(candidate.upstream_key_id)
                    .await
                    .unwrap_or(false);
                if is_available {
                    available.push(candidate);
                }
            }

            if available.is_empty() {
                // All candidates in this priority group are exhausted, try next
                continue;
            }

            // Weighted random selection from available candidates
            if let Some(selected) = weighted_random_select(&available) {
                // Now actually increment quota for the winner
                let _ = self
                    .increment_quota(selected.upstream_key_id)
                    .await;

                return Ok(SelectedUpstream {
                    upstream_key_id: selected.upstream_key_id,
                    provider_model_id: selected.provider_model_id,
                    provider_id: selected.provider_id,
                    upstream_model_name: selected.upstream_model_name.clone(),
                    base_url: selected.base_url.clone(),
                    api_format: selected.api_format.clone(),
                    config: selected.config.clone(),
                });
            }
        }

        Err(RouterError::AllUpstreamsExhausted(model_name.to_string()))
    }

    async fn get_candidates(&self, model_name: &str) -> Result<Vec<RouteCandidate>, RouterError> {
        // Check local cache first
        if let Some(cached) = self.cache.route_table.get(model_name).await {
            return Ok(cached);
        }

        // Cache miss — query DB
        let rows = sqlx::query_as::<_, RouteCandidateRow>(
            "SELECT kma.id as key_model_access_id, kma.upstream_key_id, kma.provider_model_id, \
               p.id as provider_id, pm.upstream_model_name, p.base_url, p.api_format::text as api_format, \
               kma.priority, kma.weight, pm.config \
             FROM models m \
             JOIN provider_models pm ON pm.model_id = m.id \
             JOIN key_model_access kma ON kma.provider_model_id = pm.id \
             JOIN upstream_keys uk ON uk.id = kma.upstream_key_id \
             JOIN providers p ON p.id = uk.provider_id \
             WHERE m.name = $1 AND m.is_enabled = true AND kma.is_enabled = true \
               AND uk.is_enabled = true AND uk.status != 'disabled' AND p.is_enabled = true \
             ORDER BY kma.priority ASC, kma.weight DESC",
        )
        .bind(model_name)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RouterError::Internal(e.to_string()))?;

        let candidates: Vec<RouteCandidate> = rows
            .into_iter()
            .map(|r| RouteCandidate {
                key_model_access_id: r.key_model_access_id,
                upstream_key_id: r.upstream_key_id,
                provider_model_id: r.provider_model_id,
                provider_id: r.provider_id,
                upstream_model_name: r.upstream_model_name,
                base_url: r.base_url,
                api_format: r.api_format,
                priority: r.priority,
                weight: r.weight,
                config: r.config,
            })
            .collect();

        // Store in cache
        self.cache
            .route_table
            .insert(model_name.to_string(), candidates.clone())
            .await;

        Ok(candidates)
    }

    /// Check quota availability without incrementing (uses get_usage internally).
    /// Returns true if the key has quota available (or no quota configured).
    async fn check_quota_available(&self, upstream_key_id: Uuid) -> anyhow::Result<bool> {
        // Query key_quotas for this upstream key
        let quotas = sqlx::query_as::<_, KeyQuotaRow>(
            "SELECT id, limit_value, window_seconds FROM key_quotas \
             WHERE upstream_key_id = $1",
        )
        .bind(upstream_key_id)
        .fetch_all(&self.pool)
        .await?;

        // If no quotas configured, key is always available
        if quotas.is_empty() {
            return Ok(true);
        }

        for quota in &quotas {
            let usage = self
                .quota_tracker
                .get_usage(upstream_key_id, quota.id)
                .await?;
            if usage >= quota.limit_value {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Increment quota for the selected upstream key.
    async fn increment_quota(&self, upstream_key_id: Uuid) -> anyhow::Result<()> {
        let quotas = sqlx::query_as::<_, KeyQuotaRow>(
            "SELECT id, limit_value, window_seconds FROM key_quotas \
             WHERE upstream_key_id = $1",
        )
        .bind(upstream_key_id)
        .fetch_all(&self.pool)
        .await?;

        for quota in &quotas {
            let _ = self
                .quota_tracker
                .check_and_increment(
                    upstream_key_id,
                    quota.id,
                    quota.limit_value,
                    quota.window_seconds,
                )
                .await?;
        }

        Ok(())
    }
}

fn weighted_random_select<'a>(candidates: &[&'a RouteCandidate]) -> Option<&'a RouteCandidate> {
    if candidates.is_empty() {
        return None;
    }

    let total_weight: i32 = candidates.iter().map(|c| c.weight.max(1)).sum();
    let mut target = rand::thread_rng().gen_range(0..total_weight);

    for candidate in candidates {
        let w = candidate.weight.max(1);
        if target < w {
            return Some(candidate);
        }
        target -= w;
    }

    // Fallback (shouldn't happen)
    candidates.last().copied()
}

// Internal row types for sqlx
#[derive(sqlx::FromRow)]
struct RouteCandidateRow {
    key_model_access_id: Uuid,
    upstream_key_id: Uuid,
    provider_model_id: Uuid,
    provider_id: Uuid,
    upstream_model_name: String,
    base_url: String,
    api_format: String,
    priority: i32,
    weight: i32,
    config: serde_json::Value,
}

#[derive(sqlx::FromRow)]
struct KeyQuotaRow {
    id: Uuid,
    limit_value: i32,
    window_seconds: i32,
}

pub struct SelectedUpstream {
    pub upstream_key_id: Uuid,
    pub provider_model_id: Uuid,
    pub provider_id: Uuid,
    pub upstream_model_name: String,
    pub base_url: String,
    pub api_format: String,
    pub config: serde_json::Value,
}

#[derive(Debug, thiserror::Error)]
pub enum RouterError {
    #[error("no route found for model: {0}")]
    NoRouteFound(String),
    #[error("all upstreams exhausted for model: {0}")]
    AllUpstreamsExhausted(String),
    #[error("internal error: {0}")]
    Internal(String),
}
