use dashmap::DashMap;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// One-minute time bucket of quality data.
#[derive(Debug, Clone)]
struct TimeBucket {
    /// Unix timestamp (seconds) when this bucket starts.
    minute_ts: u64,
    success_count: u64,
    failure_count: u64,
    total_latency_ms: u64,
}

impl TimeBucket {
    fn new(minute_ts: u64) -> Self {
        Self {
            minute_ts,
            success_count: 0,
            failure_count: 0,
            total_latency_ms: 0,
        }
    }

    fn total_requests(&self) -> u64 {
        self.success_count + self.failure_count
    }
}

/// 1-hour sliding window made up of 1-minute buckets.
struct SlidingWindow {
    buckets: VecDeque<TimeBucket>,
}

impl SlidingWindow {
    fn new() -> Self {
        Self {
            buckets: VecDeque::new(),
        }
    }

    fn current_minute() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            / 60
    }

    /// Remove buckets outside the 1-hour window.
    fn evict_old(&mut self) {
        let cutoff = Self::current_minute().saturating_sub(60);
        while let Some(front) = self.buckets.front() {
            if front.minute_ts < cutoff {
                self.buckets.pop_front();
            } else {
                break;
            }
        }
    }

    /// Get or create the bucket for the current minute.
    fn current_bucket_mut(&mut self) -> &mut TimeBucket {
        let minute = Self::current_minute();
        if self
            .buckets
            .back()
            .map_or(true, |b| b.minute_ts != minute)
        {
            self.buckets.push_back(TimeBucket::new(minute));
        }
        self.buckets.back_mut().unwrap()
    }

    fn record(&mut self, success: bool, latency_ms: u32) {
        self.evict_old();
        let bucket = self.current_bucket_mut();
        if success {
            bucket.success_count += 1;
        } else {
            bucket.failure_count += 1;
        }
        bucket.total_latency_ms += latency_ms as u64;
    }

    fn snapshot(&mut self) -> WindowSnapshot {
        self.evict_old();
        let mut total = 0u64;
        let mut successes = 0u64;
        let mut total_latency = 0u64;

        for b in &self.buckets {
            total += b.total_requests();
            successes += b.success_count;
            total_latency += b.total_latency_ms;
        }

        WindowSnapshot {
            total_requests: total,
            success_count: successes,
            total_latency_ms: total_latency,
        }
    }
}

struct WindowSnapshot {
    total_requests: u64,
    success_count: u64,
    total_latency_ms: u64,
}

impl WindowSnapshot {
    fn success_rate(&self) -> f64 {
        if self.total_requests == 0 {
            return 1.0;
        }
        self.success_count as f64 / self.total_requests as f64
    }

    fn avg_latency_ms(&self) -> f64 {
        if self.total_requests == 0 {
            return 0.0;
        }
        self.total_latency_ms as f64 / self.total_requests as f64
    }

    fn quality_factor(&self) -> f64 {
        let success_rate = self.success_rate();
        let avg_lat = self.avg_latency_ms();
        let latency_factor = if avg_lat < f64::EPSILON {
            1.0
        } else {
            (1000.0_f64 / avg_lat.max(100.0)).min(1.0)
        };
        success_rate * latency_factor
    }
}

/// A public snapshot of quality statistics for a provider model.
#[derive(Debug, Clone)]
pub struct QualitySnapshot {
    pub total_requests: u64,
    pub success_count: u64,
    pub failure_count: u64,
    pub success_rate: f64,
    pub avg_latency_ms: f64,
    pub quality_factor: f64,
}

/// Sliding-window quality monitor backed by in-memory DashMap.
pub struct QualityMonitor {
    windows: Arc<DashMap<Uuid, SlidingWindow>>,
}

impl QualityMonitor {
    pub fn new() -> Self {
        Self {
            windows: Arc::new(DashMap::new()),
        }
    }

    pub fn record(&self, provider_model_id: Uuid, success: bool, latency_ms: u32) {
        self.windows
            .entry(provider_model_id)
            .or_insert_with(SlidingWindow::new)
            .record(success, latency_ms);
    }

    /// Returns a quality factor in [0.0, 1.0]. Default is 1.0 when no data.
    pub fn quality_factor(&self, provider_model_id: Uuid) -> f64 {
        if let Some(mut window) = self.windows.get_mut(&provider_model_id) {
            let snap = window.snapshot();
            if snap.total_requests == 0 {
                return 1.0;
            }
            snap.quality_factor()
        } else {
            1.0
        }
    }

    pub fn stats(&self, provider_model_id: Uuid) -> Option<QualitySnapshot> {
        let mut window = self.windows.get_mut(&provider_model_id)?;
        let snap = window.snapshot();
        if snap.total_requests == 0 {
            return None;
        }
        Some(QualitySnapshot {
            total_requests: snap.total_requests,
            success_count: snap.success_count,
            failure_count: snap.total_requests - snap.success_count,
            success_rate: snap.success_rate(),
            avg_latency_ms: snap.avg_latency_ms(),
            quality_factor: snap.quality_factor(),
        })
    }
}

impl Default for QualityMonitor {
    fn default() -> Self {
        Self::new()
    }
}
