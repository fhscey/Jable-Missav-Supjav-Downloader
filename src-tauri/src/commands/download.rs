use tauri::ipc::Channel;
use tauri::State;

use crate::core::{ProgressPayload, TaskId, TaskRecord, TaskRequest};
use crate::AppState;

/// 创建单条下载任务（支持传入详情页 URL）
#[tauri::command]
pub async fn create_download_task(
    url: String,
    auto_start: Option<bool>,
    state: State<'_, AppState>,
) -> Result<TaskRecord, String> {
    // 1. 提取视频元数据
    let media = state
        .extractor
        .extract(&url, None) // TODO: 前端可能需要提供 params
        .await
        .map_err(|e| format!("Extraction failed: {}", e))?;

    // 2. 构造 TaskRequest
    let req = TaskRequest {
        title: media.title,
        stream_url: media.stream_url,
        referer: media.referer,
        source_url: Some(url),
        auto_start: auto_start.unwrap_or(false),
    };

    // 3. 提交至 DownloadManager
    let record = state
        .download_manager
        .submit(req)
        .await
        .map_err(|e| e.to_string())?;

    Ok(record)
}

/// 批量创建下载任务（接收 URL 列表）
#[tauri::command]
pub async fn create_download_tasks(
    urls: Vec<String>,
    auto_start: Option<bool>,
    state: State<'_, AppState>,
) -> Result<Vec<TaskRecord>, String> {
    let mut task_records = Vec::new();
    for url in urls {
        match create_download_task(url.clone(), auto_start, state.clone()).await {
            Ok(record) => task_records.push(record),
            Err(e) => log::error!("[create_download_tasks] 任务提交失败 [url={}]: {}", url, e),
        }
    }
    Ok(task_records)
}

/// 当 Engine 汇报流过期失效（401/403/404/410）时，通过 source_url 重新提取并更新流地址，然后继续恢复下载
pub async fn handle_stream_expired(task_id: TaskId, state: &AppState) -> Result<(), String> {
    let task = state
        .db
        .get_task(&task_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Task not found".to_string())?;

    let Some(source_url) = task.source_url else {
        log::warn!(
            "[handle_stream_expired] Task {} has no source_url, cannot refresh",
            task_id
        );
        let _ = state
            .db
            .update_status(&task_id, crate::core::model::TaskStatus::Failed);
        return Err("No source_url for task".to_string());
    };

    log::info!(
        "[handle_stream_expired] Task {} stream expired, re-extracting from: {}",
        task_id,
        source_url
    );
    let media = state
        .extractor
        .extract(&source_url, None)
        .await
        .map_err(|e| format!("Re-extraction failed: {}", e))?;

    log::info!(
        "[handle_stream_expired] Task {} remapped new stream URL: {}",
        task_id,
        media.stream_url
    );
    state
        .download_manager
        .update_stream_url(task_id, media.stream_url, media.referer)
        .await
        .map_err(|e| e.to_string())?;

    state
        .download_manager
        .resume(task_id)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// 手动启动指定下载任务
#[tauri::command]
pub async fn start_task(task_id: TaskId, state: State<'_, AppState>) -> Result<(), String> {
    state
        .download_manager
        .start(task_id)
        .await
        .map_err(|e| e.to_string())
}

/// 暂停指定下载任务
#[tauri::command]
pub async fn pause_task(task_id: TaskId, state: State<'_, AppState>) -> Result<(), String> {
    state
        .download_manager
        .pause(task_id)
        .await
        .map_err(|e| e.to_string())
}

/// 恢复指定下载任务
#[tauri::command]
pub async fn resume_task(task_id: TaskId, state: State<'_, AppState>) -> Result<(), String> {
    state
        .download_manager
        .resume(task_id)
        .await
        .map_err(|e| e.to_string())
}

/// 重试失败的下载任务
#[tauri::command]
pub async fn retry_task(task_id: TaskId, state: State<'_, AppState>) -> Result<(), String> {
    state
        .download_manager
        .retry(task_id)
        .await
        .map_err(|e| e.to_string())
}

/// 移除指定任务（中止执行、删除DB记录、可选清理本地文件）
#[tauri::command]
pub async fn remove_task(
    task_id: TaskId,
    delete_file: Option<bool>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .download_manager
        .remove(task_id, delete_file.unwrap_or(false))
        .await
        .map_err(|e| e.to_string())
}

/// 修复补全缺失分片的任务
#[tauri::command]
pub async fn repair_task(task_id: TaskId, state: State<'_, AppState>) -> Result<(), String> {
    state
        .download_manager
        .repair(task_id)
        .await
        .map_err(|e| e.to_string())
}

/// 获取单个任务的持久化记录
#[tauri::command]
pub async fn get_task(
    task_id: TaskId,
    state: State<'_, AppState>,
) -> Result<Option<TaskRecord>, String> {
    state.db.get_task(&task_id).map_err(|e| e.to_string())
}

/// 获取所有任务记录
#[tauri::command]
pub async fn get_all_tasks(state: State<'_, AppState>) -> Result<Vec<TaskRecord>, String> {
    state.db.get_all_tasks().map_err(|e| e.to_string())
}

/// 订阅高频下载进度通道
#[tauri::command]
pub async fn subscribe_progress(
    chan: Channel<ProgressPayload>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let callback = std::sync::Arc::new(move |payload| {
        let _ = chan.send(payload);
    });
    state
        .download_manager
        .subscribe_progress(callback)
        .await
        .map_err(|e| e.to_string())
}

/// 取消订阅高频下载进度通道
#[tauri::command]
pub async fn unsubscribe_progress(state: State<'_, AppState>) -> Result<(), String> {
    state
        .download_manager
        .unsubscribe_progress()
        .await
        .map_err(|e| e.to_string())
}

/// 更新任务的流地址（适用于因鉴权过期手动或自动刷新）
#[tauri::command]
pub async fn update_task_stream_url(
    task_id: TaskId,
    stream_url: String,
    referer: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .download_manager
        .update_stream_url(task_id, stream_url, referer)
        .await
        .map_err(|e| e.to_string())
}
