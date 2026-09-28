pub mod hls;
pub mod http;

pub use hls::{HlsEngine, M3u8Engine};
pub use http::HttpEngine;

use std::sync::Arc;
use tokio::sync::{mpsc, OwnedSemaphorePermit};

use crate::config::PreferredQuality;
use crate::core::db::Database;
use crate::core::model::{ProgressPayload, TaskId, TaskRecord, TaskStatus};
use crate::core::network::HttpClient;
use crate::core::throttler::RateThrottler;

/// 纯 Rust 高频进度推送闭包抽象
pub type ProgressCallback = Arc<dyn Fn(ProgressPayload) + Send + Sync>;

/// 进度回调共享指针
pub type ProgressHub = Arc<parking_lot::RwLock<Option<ProgressCallback>>>;

/// Engine 层向 Manager 上报的内部事件
#[derive(Debug)]
pub enum EngineEvent {
    StatusChanged(TaskId, TaskStatus),
    StreamExpired(TaskId, u16),
}

/// Engine 层纯净执行原语
#[derive(Debug)]
pub enum EngineCommand {
    /// 授权启动或继续执行（携带全局并发槽位 Permit）
    Start(OwnedSemaphorePermit),
    /// 挂起下载并释放并发槽位
    Pause,
    /// 热替换流地址与 Referer（切片重映射续传）
    UpdateStreamUrl { stream_url: String, referer: Option<String> },
    /// 终止任务并退出 Engine 协程
    Stop { delete_file: bool },
}

pub trait DownloadEngine {
    fn spawn(
        ctx: EngineContext,
        cmd_rx: mpsc::Receiver<EngineCommand>,
        event_tx: mpsc::Sender<EngineEvent>,
    );
}

pub struct EngineContext {
    pub task: TaskRecord,
    pub client: HttpClient,
    pub progress_hub: ProgressHub,
    pub preferred_quality: PreferredQuality,
    pub throttler: Arc<RateThrottler>,
    pub db: Arc<Database>,
}
