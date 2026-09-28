#[derive(Debug, Clone)]
pub struct SpeedTracker {
    alpha: f64,
    last_bytes: u64,
    speed_ewma: f64,
}

/// 基于 EWMA（指数加权移动平均）的下载速率平滑计算工具
impl SpeedTracker {
    /// 默认 EWMA 衰减系数 (alpha = 0.25)
    pub const DEFAULT_ALPHA: f64 = 0.25;

    /// 创建默认配置的速率追踪器
    pub fn new() -> Self {
        Self::with_alpha(Self::DEFAULT_ALPHA)
    }

    /// 使用自定义权重 alpha 创建速率追踪器 (0.0 < alpha <= 1.0)
    pub fn with_alpha(alpha: f64) -> Self {
        Self {
            alpha: alpha.clamp(0.01, 1.0),
            last_bytes: 0,
            speed_ewma: 0.0,
        }
    }

    /// 传入当前累计字节数与流逝毫秒数，更新并返回平滑后的下载速度 (bytes/s)
    pub fn update(&mut self, current_bytes: u64, elapsed_ms: u64) -> u64 {
        let delta = current_bytes.saturating_sub(self.last_bytes);
        self.last_bytes = current_bytes;

        if elapsed_ms == 0 {
            return self.current_speed_bps();
        }

        let instant_speed = (delta as f64) * (1000.0 / elapsed_ms as f64);

        if self.speed_ewma <= 0.0 {
            self.speed_ewma = instant_speed;
        } else {
            self.speed_ewma = self.alpha * instant_speed + (1.0 - self.alpha) * self.speed_ewma;
        }

        if self.speed_ewma < 1.0 {
            self.speed_ewma = 0.0;
        }

        self.current_speed_bps()
    }

    pub fn current_speed_bps(&self) -> u64 {
        self.speed_ewma.round() as u64
    }

    pub fn reset(&mut self, current_bytes: u64) {
        self.last_bytes = current_bytes;
        self.speed_ewma = 0.0;
    }
}

impl Default for SpeedTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_speed_tracker_cold_start_and_smoothing() {
        let mut tracker = SpeedTracker::with_alpha(0.25);
        assert_eq!(tracker.current_speed_bps(), 0);

        let s1 = tracker.update(200_000, 200);
        assert_eq!(s1, 1_000_000);

        let s2 = tracker.update(300_000, 200);
        assert_eq!(s2, 875_000);

        tracker.reset(300_000);
        assert_eq!(tracker.current_speed_bps(), 0);

        let s3 = tracker.update(350_000, 200);
        assert_eq!(s3, 250_000);
    }

    #[test]
    fn test_speed_tracker_decay_to_zero() {
        let mut tracker = SpeedTracker::with_alpha(0.5);
        tracker.update(1000, 1000);

        for _ in 0..15 {
            tracker.update(1000, 200);
        }
        assert_eq!(tracker.current_speed_bps(), 0);
    }
}
