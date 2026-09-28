use std::path::{Path, PathBuf};

/// 净化文件名中的特殊符号，保证跨操作系统安全
pub fn sanitize_file_name(name: &str) -> String {
    let invalid_chars = ['/', '\\', '<', '>', ':', '"', '|', '?', '*', '\0'];
    let clean: String = name
        .chars()
        .map(|ch| if invalid_chars.contains(&ch) { '_' } else { ch })
        .collect();
    let trimmed = clean.trim().trim_matches('.');
    if trimmed.is_empty() {
        "video.mp4".to_string()
    } else {
        format!("{}.mp4", trimmed)
    }
}

/// 解析不重名的文件目标路径（若目标已存在，则自动添加 `(1)`, `(2)` 后缀）
pub fn resolve_unique_path(save_dir: &Path, file_name: &str) -> PathBuf {
    let target = save_dir.join(file_name);
    if !target.exists() {
        return target;
    }

    let stem = Path::new(file_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("download");
    let ext = Path::new(file_name)
        .extension()
        .and_then(|s| s.to_str())
        .map(|e| format!(".{}", e))
        .unwrap_or_default();

    let mut counter = 1;
    loop {
        let candidate_name = format!("{} ({}){}", stem, counter, ext);
        let candidate_path = save_dir.join(candidate_name);
        if !candidate_path.exists() {
            return candidate_path;
        }
        counter += 1;
    }
}

/// 原子重命名文件，跨卷/跨设备移动失败时自动降级为复制并删除原文件
pub async fn move_or_copy_file(src: &Path, dst: &Path) -> std::io::Result<()> {
    if let Err(e) = tokio::fs::rename(src, dst).await {
        log::warn!(
            "Rename file failed ({}), fallback to copy: {:?} -> {:?}",
            e,
            src,
            dst
        );
        tokio::fs::copy(src, dst).await?;
        let _ = tokio::fs::remove_file(src).await;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_file_name() {
        assert_eq!(sanitize_file_name("test/file:name*"), "test_file_name_.mp4");
        assert_eq!(sanitize_file_name("..."), "video.mp4");
        assert_eq!(sanitize_file_name("normal_title"), "normal_title.mp4");
    }

    #[test]
    fn test_resolve_unique_path() {
        let temp_dir = std::env::temp_dir().join("avdl_test_unique_path");
        let _ = std::fs::create_dir_all(&temp_dir);

        let p1 = resolve_unique_path(&temp_dir, "test.mp4");
        assert_eq!(p1, temp_dir.join("test.mp4"));

        std::fs::write(&p1, b"hello").unwrap();
        let p2 = resolve_unique_path(&temp_dir, "test.mp4");
        assert_eq!(p2, temp_dir.join("test (1).mp4"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_move_or_copy_file() {
        let temp_dir = std::env::temp_dir().join("avdl_test_move_file");
        let _ = tokio::fs::create_dir_all(&temp_dir).await;

        let src = temp_dir.join("src.txt");
        let dst = temp_dir.join("dst.txt");
        tokio::fs::write(&src, b"test content").await.unwrap();

        let res = move_or_copy_file(&src, &dst).await;
        assert!(res.is_ok());
        assert!(!src.exists());
        assert!(dst.exists());
        let content = tokio::fs::read(&dst).await.unwrap();
        assert_eq!(content, b"test content");

        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }
}
