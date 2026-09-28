use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use parking_lot::Mutex;

use crate::core::error::CoreResult;

struct ThrottlerState {
    tokens: f64,
    last_replenished: Instant,
}

/// 全局令牌桶限速器（支持动态热修改限速，0 表示不限速）
#[derive(Clone)]
pub struct RateThrottler {
    bytes_per_sec: Arc<AtomicU64>,
    state: Arc<Mutex<ThrottlerState>>,
}

impl RateThrottler {
    pub fn new(bytes_per_sec: u64) -> Self {
        Self {
            bytes_per_sec: Arc::new(AtomicU64::new(bytes_per_sec)),
            state: Arc::new(Mutex::new(ThrottlerState {
                tokens: 0.0,
                last_replenished: Instant::now(),
            })),
        }
    }

    /// 设置新的最大速率 (bytes/s)，0 为不限速
    pub fn set_rate_limit(&self, bytes_per_sec: u64) {
        self.bytes_per_sec.store(bytes_per_sec, Ordering::Relaxed);
        let mut state = self.state.lock();
        state.tokens = 0.0;
        state.last_replenished = Instant::now();
    }

    /// 获取当前限速值 (bytes/s)
    pub fn get_rate_limit(&self) -> u64 {
        self.bytes_per_sec.load(Ordering::Relaxed)
    }

    /// 申请消费指定字节配额，如果超出速率则异步睡眠等待
    pub async fn acquire(&self, bytes: usize) -> CoreResult<()> {
        let limit = self.bytes_per_sec.load(Ordering::Relaxed);
        if limit == 0 || bytes == 0 {
            return Ok(());
        }

        let wait_duration = {
            let mut state = self.state.lock();
            let now = Instant::now();
            let elapsed = now.duration_since(state.last_replenished).as_secs_f64();
            state.last_replenished = now;

            let max_bucket = ((limit as f64) * 0.5).max(64.0 * 1024.0);
            state.tokens = (state.tokens + elapsed * (limit as f64)).min(max_bucket);

            state.tokens -= bytes as f64;

            if state.tokens < 0.0 {
                let wait_secs = (-state.tokens) / (limit as f64);
                Some(Duration::from_secs_f64(wait_secs))
            } else {
                None
            }
        };

        if let Some(duration) = wait_duration {
            tokio::time::sleep(duration).await;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_concurrent_throttling() {
        let limit = 500_000; // 500 KB/s
        let throttler = Arc::new(RateThrottler::new(limit));

        let start = Instant::now();
        let mut handles = Vec::new();
        for _ in 0..5 {
            let t = throttler.clone();
            handles.push(tokio::spawn(async move {
                t.acquire(100_000).await.unwrap();
            }));
        }

        for h in handles {
            h.await.unwrap();
        }

        let elapsed = start.elapsed();
        assert!(
            elapsed >= Duration::from_millis(600),
            "Expected throttling to take >= 600ms, but took {:?}",
            elapsed
        );
    }
}
