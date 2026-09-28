pub mod db;
pub mod engine;
pub mod error;
pub mod manager;
pub mod model;
pub mod network;
pub mod protocol;
pub mod speed;
pub mod throttler;
pub mod worker;

// 常用顶级再导出，简化下游使用
pub use db::{Database, DatabaseError};
pub use error::{CoreError, CoreResult};
pub use manager::{spawn_download_manager, DownloadManagerHandle};
pub use model::{ProgressPayload, TaskEvent, TaskId, TaskRecord, TaskRequest, TaskStatus};
pub use network::{HttpClient, NetworkError, ProxyMode, DEFAULT_UA};
pub use speed::SpeedTracker;
pub use throttler::RateThrottler;

