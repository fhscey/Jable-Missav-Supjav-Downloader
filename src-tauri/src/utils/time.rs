use std::time::{SystemTime, UNIX_EPOCH};

/// 获取当前 UNIX 时间戳（秒）
#[inline]
pub fn current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// 获取当前 UNIX 时间戳（毫秒）
#[inline]
pub fn current_timestamp_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
