use tauri::State;

use crate::config::AppConfig;
use crate::AppState;

/// 获取当前配置
#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    Ok(state.config.read().clone())
}

/// 保存并落盘配置（同时动态更新调度器最大并发和代理）
#[tauri::command]
pub async fn set_config(config: AppConfig, state: State<'_, AppState>) -> Result<(), String> {
    let old_proxy_mode = state.config.read().proxy_mode.clone();
    if old_proxy_mode != config.proxy_mode {
        let mut guard = state.http_client.write();
        guard
            .set_proxy(&config.proxy_mode)
            .map_err(|e| format!("Failed to set proxy for client: {}", e))?;
    }

    let _ = state
        .download_manager
        .update_rate_limit(config.max_download_speed)
        .await;

    // 同步更新站点可用域名至 ExtractorService（实时重新生成并缓存 Manifest）
    for (site, domains) in &config.site_domains {
        let _ = state.extractor.update_site_domains(*site, domains.clone());
    }

    config.save()?;
    let mut guard = state.config.write();
    *guard = config;
    Ok(())
}
