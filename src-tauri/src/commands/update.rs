use tauri::State;

use crate::updater::UpdateInfo;
use crate::AppState;

/// 检查应用更新
#[tauri::command]
pub async fn check_for_update(state: State<'_, AppState>) -> Result<UpdateInfo, String> {
    state.updater.check().await
}
