use std::path::PathBuf;
use sysinfo::Disks;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct DiskSpace {
    pub total: u64,
    pub free: u64,
}

/// 调起系统原生文件夹选择弹窗，选择下载保存目录
#[tauri::command]
pub async fn select_directory() -> Result<Option<String>, String> {
    let folder = rfd::AsyncFileDialog::new().pick_folder().await;
    Ok(folder.map(|h| h.path().to_string_lossy().to_string()))
}

/// 查询指定路径所在磁盘的容量信息（总容量与剩余可用容量）
#[tauri::command]
pub async fn get_disk_space(path: String) -> Result<DiskSpace, String> {
    tokio::task::spawn_blocking(move || {
        let mut check_path = PathBuf::from(&path);
        while !check_path.exists() {
            if let Some(parent) = check_path.parent() {
                if parent == check_path {
                    break;
                }
                check_path = parent.to_path_buf();
            } else {
                break;
            }
        }

        let resolved = check_path.canonicalize().unwrap_or(check_path);
        let disks = Disks::new_with_refreshed_list();
        let mut best_match = None;
        let mut best_len = 0;

        for disk in disks.list() {
            let mp = disk.mount_point();
            if resolved.starts_with(mp) {
                let len = mp.as_os_str().len();
                if len >= best_len {
                    best_len = len;
                    best_match = Some((disk.total_space(), disk.available_space()));
                }
            }
        }

        if let Some((total, free)) = best_match {
            Ok(DiskSpace { total, free })
        } else if let Some(disk) = disks.list().first() {
            Ok(DiskSpace {
                total: disk.total_space(),
                free: disk.available_space(),
            })
        } else {
            Err("No disk found".to_string())
        }
    })
    .await
    .map_err(|e| e.to_string())?
}
