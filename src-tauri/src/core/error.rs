use std::path::PathBuf;
use thiserror::Error;

use crate::core::db::DatabaseError;
use crate::core::model::TaskId;
use crate::core::network::NetworkError;
use crate::utils::RemuxError;

pub type CoreResult<T> = Result<T, CoreError>;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("Invalid URL: {0}")]
    InvalidUrl(#[from] url::ParseError),

    #[error("Network error: {0}")]
    Network(#[from] NetworkError),

    #[error("HTTP request error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("HTTP status error: {0}")]
    HttpStatus(u16),

    #[error("Stream expired or unauthorized (HTTP {0})")]
    StreamExpired(u16),

    #[error("Failed to parse M3U8 playlist: {0}")]
    M3u8Parse(String),

    #[error("No valid variant stream found in master playlist")]
    NoValidVariant,

    #[error("Nested master playlist is not supported")]
    NestedMasterPlaylist,

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization/deserialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),

    #[error("Task {0} not found")]
    TaskNotFound(TaskId),

    #[error("No valid segments found to merge at {0:?}")]
    MergeNoValidSegments(PathBuf),

    #[error("Worker failed for {url} chunk {chunk_id}: {reason}")]
    WorkerFailed {
        url: String,
        chunk_id: usize,
        reason: String,
    },

    #[error("Rate throttler error: {0}")]
    Throttler(String),

    #[error(transparent)]
    Remux(#[from] RemuxError),

    #[error("Channel closed: {0}")]
    ChannelClosed(String),

    #[error("Oneshot recv error: {0}")]
    OneshotRecv(#[from] tokio::sync::oneshot::error::RecvError),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_core_error_display() {
        let err = CoreError::NoValidVariant;
        assert_eq!(
            err.to_string(),
            "No valid variant stream found in master playlist"
        );

        let err = CoreError::NestedMasterPlaylist;
        assert_eq!(err.to_string(), "Nested master playlist is not supported");

        let err = CoreError::MergeNoValidSegments(PathBuf::from("/tmp/merge.tmp"));
        assert_eq!(
            err.to_string(),
            "No valid segments found to merge at \"/tmp/merge.tmp\""
        );
    }

    #[test]
    fn test_core_error_from_url() {
        let bad_url = "not a url";
        let parse_res: Result<url::Url, url::ParseError> = url::Url::parse(bad_url);
        let core_err: CoreError = parse_res.unwrap_err().into();
        assert!(matches!(core_err, CoreError::InvalidUrl(_)));
    }
}
