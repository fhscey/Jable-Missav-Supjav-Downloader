pub mod commands;
pub mod config;
pub mod core;
pub mod extractor;
pub mod network;
pub mod updater;
pub mod utils;

use parking_lot::RwLock;
use std::sync::Arc;
use tauri::Manager;

use crate::config::AppConfig;
use crate::core::{spawn_download_manager, Database, DownloadManagerHandle, HttpClient, TaskEvent};
use crate::extractor::ExtractService;
use crate::network::{stream, WebviewEngine};
use crate::updater::Updater;

#[derive(Clone)]
pub struct AppState {
    pub http_client: Arc<RwLock<HttpClient>>,
    pub webview_engine: WebviewEngine,
    pub download_manager: DownloadManagerHandle,
    pub config: Arc<RwLock<AppConfig>>,
    pub extractor: Arc<ExtractService>,
    pub updater: Arc<Updater>,
    pub db: Arc<Database>,
}

impl AppState {
    pub fn get_config(&self) -> AppConfig {
        self.config.try_read().unwrap().clone()
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .register_asynchronous_uri_scheme_protocol("stream", |ctx, request, responder| {
            let app_handle = ctx.app_handle().clone();
            tauri::async_runtime::spawn(async move {
                let resp = match stream::handle_stream_request(app_handle, request).await {
                    Ok(resp) => resp,
                    Err(e) => {
                        log::error!("[stream protocol] 处理失败: err = {}", e);
                        tauri::http::Response::builder()
                            .status(500)
                            .header("Content-Type", "text/plain")
                            .header("Access-Control-Allow-Origin", "*")
                            .body("Streaming proxy failed".as_bytes().to_vec())
                            .unwrap()
                    }
                };
                let _ = responder.respond(resp);
            });
        })
        .plugin(tauri_plugin_opener::init())
        .plugin({
            let app_log_level = if cfg!(debug_assertions) {
                log::LevelFilter::Debug
            } else {
                log::LevelFilter::Info
            };

            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .level_for("avdl", app_log_level)
                .level_for("avdl_lib", app_log_level)
                .build()
        })
        .setup(|app| {
            // 启动一个后台 Tokio 运行时，避免与 Tauri 运行时冲突
            let _runtime_guard = tauri::async_runtime::handle().inner().enter();

            let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;

            let config = AppConfig::load_or_init(&app_data_dir)
                .map_err(|e| format!("Failed to initialize config: {}", e))?;
            let config_arc = Arc::new(RwLock::new(config.clone()));

            let app_handle = app.app_handle();

            let client = HttpClient::new(&config.proxy_mode)
                .map_err(|e| format!("Failed to initialize client: {}", e))?;
            let client_arc = Arc::new(RwLock::new(client));

            let db_path = app_data_dir.join("avdl.db");
            let database = Database::new(&db_path)
                .map_err(|e| format!("Failed to open database at {:?}: {}", db_path, e))?;
            database
                .init()
                .map_err(|e| format!("Failed to initialize database: {}", e))?;
            let db_arc = Arc::new(database);

            let download_manager = spawn_download_manager(
                config_arc.clone(),
                client_arc.clone(),
                db_arc.clone(),
            );

            let webview_engine = WebviewEngine::new(&app_handle);
            let extractor = Arc::new(ExtractService::new(
                client_arc.clone(),
                webview_engine.clone(),
                config_arc.clone(),
            ));

            let updater = Arc::new(Updater::new(client_arc.clone()));

            let app_state = AppState {
                http_client: client_arc,
                webview_engine,
                download_manager: download_manager.clone(),
                config: config_arc,
                extractor,
                updater,
                db: db_arc,
            };

            // 监听 manager 层发出的领域事件，桥接并转发给 Tauri Webview 前端
            let mut event_rx = download_manager.subscribe_events();
            let app_handle_for_events = app.handle().clone();
            let state_for_events = app_state.clone();
            tauri::async_runtime::spawn(async move {
                use tauri::Emitter;
                while let Ok(event) = event_rx.recv().await {
                    match event {
                        TaskEvent::StatusChanged { task_id, status } => {
                            let _ = app_handle_for_events.emit(
                                "task_status_changed",
                                serde_json::json!({
                                     "task_id": task_id,
                                     "status": status,
                                }),
                            );
                        }
                        TaskEvent::StreamExpired { task_id, status_code } => {
                            log::warn!("Task {} stream expired (HTTP {}), re-extracting detail...", task_id, status_code);
                            let st = state_for_events.clone();
                            tauri::async_runtime::spawn(async move {
                                if let Err(e) = commands::handle_stream_expired(task_id, &st).await {
                                    log::error!("Failed to auto refresh expired stream for task {}: {}", task_id, e);
                                }
                            });
                        }
                    }
                }
            });

            app.manage(app_state);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // 配置
            commands::get_config,
            commands::set_config,
            // 文件系统(打开文件夹已用 tauri_plugin_opener)
            commands::select_directory,
            commands::get_disk_space,
            // 检查更新
            commands::check_for_update,
            // 下载任务
            commands::create_download_task,
            commands::create_download_tasks,
            commands::start_task,
            commands::pause_task,
            commands::resume_task,
            commands::retry_task,
            commands::remove_task,
            commands::get_task,
            commands::get_all_tasks,
            commands::repair_task,
            commands::subscribe_progress,
            commands::unsubscribe_progress,
            commands::update_task_stream_url,
            // 浏览与解析
            commands::extract_media,
            commands::get_site_manifests,
            commands::list_videos,
            commands::search_videos,
            commands::get_preview_stream_url,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                if window.label() == "main" {
                    log::info!(
                        "Main window close requested, closing background webview and exiting..."
                    );
                    if let Some(cf_win) = window.app_handle().get_webview_window("cf-window") {
                        let _ = cf_win.destroy();
                    }
                    window.app_handle().exit(0);
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
