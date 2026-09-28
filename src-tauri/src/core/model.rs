use std::path::PathBuf;
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use serde::{Deserialize, Serialize};

pub type TaskId = uuid::Uuid;

/// DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRequest {
    pub title: String,
    pub stream_url: String,
    pub referer: Option<String>,
    pub source_url: Option<String>,
    pub auto_start: bool,
}

/// 任务状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Running,
    Paused,
    Completed,  // 完美合并所有切片，可以正常播放
    Incomplete, // 部分切片缺失或错误，但最终文件可以播放
    Failed,     // 失败，没有最终文件或者无法播放
}

impl ToSql for TaskStatus {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        let s = match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Paused => "paused",
            Self::Completed => "completed",
            Self::Incomplete => "incomplete",
            Self::Failed => "failed",
        };
        Ok(s.into())
    }
}

impl FromSql for TaskStatus {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let s = value.as_str()?;
        match s {
            "pending" => Ok(Self::Pending),
            "running" => Ok(Self::Running),
            "paused" => Ok(Self::Paused),
            "completed" => Ok(Self::Completed),
            "incomplete" => Ok(Self::Incomplete),
            "failed" => Ok(Self::Failed),
            _ => Err(FromSqlError::Other(
                format!("Invalid TaskStatus value: '{s}'").into(),
            )),
        }
    }
}

/// Entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRecord {
    pub id: TaskId,
    pub title: String,
    pub stream_url: String,
    pub referer: Option<String>,
    pub source_url: Option<String>,
    pub save_dir: PathBuf,
    pub status: TaskStatus,
    pub completed_segments: Option<usize>,
    pub total_segments: Option<usize>,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// 进度更新消息载荷（纯运行时指标数据源）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressPayload {
    pub task_id: TaskId,
    pub downloaded_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
    pub completed_segments: Option<usize>,
    pub total_segments: Option<usize>,
    pub speed_bps: u64,
}

/// 核心下载调度器向外界派发的领域事件
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum TaskEvent {
    #[serde(rename = "task_status_changed")]
    StatusChanged { task_id: TaskId, status: TaskStatus },
    #[serde(rename = "stream_expired")]
    StreamExpired { task_id: TaskId, status_code: u16 },
}
