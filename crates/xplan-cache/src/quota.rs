use deadpool_redis::Pool;
use redis::AsyncCommands;
use uuid::Uuid;

pub struct QuotaTracker {
    pool: Pool,
}

impl QuotaTracker {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    pub async fn check_and_increment(
        &self,
        upstream_key_id: Uuid,
        quota_id: Uuid,
        limit: i32,
        window_seconds: i32,
    ) -> anyhow::Result<QuotaCheckResult> {
        let key = format!("xplan:quota:{}:{}", upstream_key_id, quota_id);
        let mut conn = self.pool.get().await?;

        let current: Option<i32> = conn.get(&key).await?;
        let current = current.unwrap_or(0);

        if current >= limit {
            return Ok(QuotaCheckResult::Exhausted { current, limit });
        }

        let new_val: i32 = conn.incr(&key, 1).await?;

        if new_val == 1 {
            let _: () = conn.expire(&key, window_seconds as i64).await?;
        }

        Ok(QuotaCheckResult::Allowed {
            current: new_val,
            limit,
        })
    }

    pub async fn get_usage(
        &self,
        upstream_key_id: Uuid,
        quota_id: Uuid,
    ) -> anyhow::Result<i32> {
        let key = format!("xplan:quota:{}:{}", upstream_key_id, quota_id);
        let mut conn = self.pool.get().await?;
        let val: Option<i32> = conn.get(&key).await?;
        Ok(val.unwrap_or(0))
    }

    pub async fn get_ttl(
        &self,
        upstream_key_id: Uuid,
        quota_id: Uuid,
    ) -> anyhow::Result<i64> {
        let key = format!("xplan:quota:{}:{}", upstream_key_id, quota_id);
        let mut conn = self.pool.get().await?;
        let ttl: i64 = redis::cmd("TTL").arg(&key).query_async(&mut *conn).await?;
        Ok(ttl)
    }
}

#[derive(Debug)]
pub enum QuotaCheckResult {
    Allowed { current: i32, limit: i32 },
    Exhausted { current: i32, limit: i32 },
}

impl QuotaCheckResult {
    pub fn is_allowed(&self) -> bool {
        matches!(self, QuotaCheckResult::Allowed { .. })
    }
}
