use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RemuxError {
    #[error("系统中未检测到 FFmpeg（未安装或不在常见路径中）。请在系统中安装 FFmpeg 以进行无损视频封装。")]
    FfmpegNotFound,

    #[error("FFmpeg 执行失败 (退出码: {code:?}): {stderr}")]
    ExecutionFailed {
        code: Option<i32>,
        stderr: String,
    },

    #[error("FFmpeg 执行完毕但输出文件为空: {0:?}")]
    OutputEmpty(PathBuf),

    #[error("调用 FFmpeg 进程启动异常: {io_err} (路径: {path:?})")]
    ProcessSpawnFailed {
        path: PathBuf,
        io_err: String,
    },

    #[error("I/O 错误: {0}")]
    Io(#[from] std::io::Error),
}

pub type RemuxResult<T> = Result<T, RemuxError>;

/// 检测系统中可用的 FFmpeg 可执行文件路径
pub fn find_ffmpeg() -> Option<PathBuf> {
    let common_paths = [
        "/opt/homebrew/bin/ffmpeg",
        "/usr/local/bin/ffmpeg",
        "/usr/bin/ffmpeg",
        "/bin/ffmpeg",
        "/usr/local/opt/ffmpeg/bin/ffmpeg",
    ];

    for p in common_paths {
        let pb = PathBuf::from(p);
        if pb.is_file() {
            return Some(pb);
        }
    }

    if let Ok(output) = std::process::Command::new("ffmpeg")
        .arg("-version")
        .output()
    {
        if output.status.success() {
            return Some(PathBuf::from("ffmpeg"));
        }
    }

    None
}

/// 将 MPEG-TS 临时文件通过 FFmpeg 无损转封装为标准 MP4 容器
pub async fn remux_ts_to_mp4(ts_path: &Path, mp4_path: &Path) -> RemuxResult<()> {
    let in_len = tokio::fs::metadata(ts_path)
        .await
        .map(|m| m.len())
        .unwrap_or(0);

    let ffmpeg_bin = find_ffmpeg().ok_or(RemuxError::FfmpegNotFound)?;

    log::info!(
        "Using FFmpeg at {:?} to remux TS to MP4: {:?} -> {:?}",
        ffmpeg_bin,
        ts_path,
        mp4_path
    );

    let mut cmd = tokio::process::Command::new(&ffmpeg_bin);
    cmd.arg("-y")
        .arg("-f")
        .arg("mpegts")
        .arg("-analyzeduration")
        .arg("100M")
        .arg("-probesize")
        .arg("100M")
        .arg("-i")
        .arg(ts_path)
        .arg("-c")
        .arg("copy")
        .arg("-movflags")
        .arg("+faststart")
        .arg(mp4_path);

    match cmd.output().await {
        Ok(output) if output.status.success() => {
            let out_len = tokio::fs::metadata(mp4_path)
                .await
                .map(|m| m.len())
                .unwrap_or(0);

            if out_len > 0 {
                log::info!(
                    "FFmpeg remuxed successfully: {:?} (input: {} MB, output: {} MB)",
                    mp4_path,
                    in_len / (1024 * 1024),
                    out_len / (1024 * 1024)
                );
                let _ = tokio::fs::remove_file(ts_path).await;
                Ok(())
            } else {
                let _ = tokio::fs::remove_file(mp4_path).await;
                log::error!("FFmpeg succeeded but output file is empty: {:?}", mp4_path);
                Err(RemuxError::OutputEmpty(mp4_path.to_path_buf()))
            }
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let _ = tokio::fs::remove_file(mp4_path).await;
            log::error!("FFmpeg remux failed: {}", stderr);
            Err(RemuxError::ExecutionFailed {
                code: output.status.code(),
                stderr,
            })
        }
        Err(e) => {
            let _ = tokio::fs::remove_file(mp4_path).await;
            log::error!("Failed to spawn FFmpeg ({:?}): {}", ffmpeg_bin, e);
            Err(RemuxError::ProcessSpawnFailed {
                path: ffmpeg_bin,
                io_err: e.to_string(),
            })
        }
    }
}

const TS_SYNC_BYTE: u8 = 0x47;
const TS_PACKET_SIZE: usize = 188;
const MAX_SEARCH_OFFSET: usize = 64 * 1024;
const VERIFY_PACKETS: usize = 3;

pub fn strip_image_disguise(data: &[u8]) -> &[u8] {
    let scan_window = &data[..data.len().min(MAX_SEARCH_OFFSET)];

    let valid_offset = scan_window
        .iter()
        .enumerate()
        .filter(|&(_, &byte)| byte == TS_SYNC_BYTE)
        .map(|(idx, _)| idx)
        .find(|&offset| is_valid_ts_start(&data[offset..]));

    match valid_offset {
        Some(offset) => &data[offset..],
        None => data,
    }
}

fn is_valid_ts_start(candidate: &[u8]) -> bool {
    let chunks = candidate.chunks_exact(TS_PACKET_SIZE).take(VERIFY_PACKETS);

    let mut checked_chunks = 0;
    for chunk in chunks {
        if chunk.first() != Some(&TS_SYNC_BYTE) {
            return false;
        }
        checked_chunks += 1;
    }

    checked_chunks > 0 || candidate.first() == Some(&TS_SYNC_BYTE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_ffmpeg() {
        if let Some(path) = find_ffmpeg() {
            assert!(!path.as_os_str().is_empty());
        }
    }

    #[test]
    fn test_strip_image_disguise_pure_ts() {
        let mut packet = vec![0u8; 188 * 3];
        packet[0] = TS_SYNC_BYTE;
        packet[188] = TS_SYNC_BYTE;
        packet[376] = TS_SYNC_BYTE;

        let stripped = strip_image_disguise(&packet);
        assert_eq!(stripped.len(), packet.len());
        assert_eq!(stripped[0], TS_SYNC_BYTE);
    }

    #[test]
    fn test_strip_image_disguise_with_png_header() {
        let mut data = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        let mut ts_part = vec![0u8; 188 * 3];
        ts_part[0] = TS_SYNC_BYTE;
        ts_part[188] = TS_SYNC_BYTE;
        ts_part[376] = TS_SYNC_BYTE;

        data.extend_from_slice(&ts_part);

        let stripped = strip_image_disguise(&data);
        assert_eq!(stripped.len(), ts_part.len());
        assert_eq!(stripped[0], TS_SYNC_BYTE);
    }
}
