use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use tokio::sync::{broadcast, mpsc, oneshot, Semaphore};

use super::engine::{
    hls::HlsEngine, http::HttpEngine, DownloadEngine, EngineCommand, EngineContext, EngineEvent,
    ProgressCallback, ProgressHub,
};
use super::model::{TaskEvent, TaskId, TaskRecord, TaskRequest, TaskStatus};
use super::protocol::hls::is_m3u8;
use super::throttler::RateThrottler;
use crate::config::AppConfig;
use crate::core::db::Database;
use crate::core::error::{CoreError, CoreResult};
use crate::core::network::HttpClient;
use crate::utils::current_timestamp;

use parking_lot::RwLock;

/// 用户下载业务领域命令
enum DownloadCommand {
    /// 提交新任务（由 task.auto_start 决定是否立即进入下载排队队列）
    Submit {
        task_id: TaskId,
        task: TaskRequest,
        reply: oneshot::Sender<CoreResult<TaskRecord>>,
    },
    /// 手动启动待命/排队中的任务
    Start {
        task_id: TaskId,
        reply: oneshot::Sender<CoreResult<()>>,
    },
    /// 暂停正在下载的任务并释放并发配额
    Pause {
        task_id: TaskId,
        reply: oneshot::Sender<CoreResult<()>>,
    },
    /// 恢复已暂停的任务
    Resume {
        task_id: TaskId,
        reply: oneshot::Sender<CoreResult<()>>,
    },
    /// 重试下载失败的任务（从断点恢复并重新进入调度）
    Retry {
        task_id: TaskId,
        reply: oneshot::Sender<CoreResult<()>>,
    },
    /// 移除/删除任务（终止底层引擎、清除数据库记录，并可选清理磁盘文件）
    Remove {
        task_id: TaskId,
        delete_file: bool,
        reply: oneshot::Sender<CoreResult<()>>,
    },
    /// 重新修复补全缺失分片（仅针对 Incomplete 任务）
    Repair {
        task_id: TaskId,
        reply: oneshot::Sender<CoreResult<()>>,
    },
    /// 更新指定任务的播放流地址与 Referer（热替换切片续传）
    UpdateStreamUrl {
        task_id: TaskId,
        stream_url: String,
        referer: Option<String>,
        reply: oneshot::Sender<CoreResult<()>>,
    },
    /// 订阅高频进度流（纯 Rust 闭包回调）
    SubscribeProgress { callback: ProgressCallback },
    /// 取消订阅高频进度流
    UnsubscribeProgress,
    /// 动态热调整全局限速带宽 (0 表示不限速)
    UpdateRateLimit { bytes_per_sec: u64 },
}

impl From<mpsc::error::SendError<DownloadCommand>> for CoreError {
    fn from(err: mpsc::error::SendError<DownloadCommand>) -> Self {
        CoreError::ChannelClosed(err.to_string())
    }
}

/// 外部与下载管理器交互的线程安全轻量句柄
#[derive(Clone)]
pub struct DownloadManagerHandle {
    cmd_tx: mpsc::Sender<DownloadCommand>,
    event_tx: broadcast::Sender<TaskEvent>,
}

impl DownloadManagerHandle {
    fn new(cmd_tx: mpsc::Sender<DownloadCommand>, event_tx: broadcast::Sender<TaskEvent>) -> Self {
        Self { cmd_tx, event_tx }
    }

    /// 统一异步 RPC 调用：自动注入 oneshot
    async fn call<T>(
        &self,
        make_cmd: impl FnOnce(oneshot::Sender<CoreResult<T>>) -> DownloadCommand,
    ) -> CoreResult<T> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.cmd_tx.send(make_cmd(reply_tx)).await?;
        reply_rx.await?
    }

    /// 订阅核心层的领域事件流（状态变更等）
    pub fn subscribe_events(&self) -> broadcast::Receiver<TaskEvent> {
        self.event_tx.subscribe()
    }

    /// 提交新下载任务，返回生成的 TaskRecord 实体
    pub async fn submit(&self, task: TaskRequest) -> CoreResult<TaskRecord> {
        let task_id = TaskId::new_v4();
        self.call(|reply| DownloadCommand::Submit {
            task_id,
            task,
            reply,
        })
        .await
    }

    /// 手动启动指定任务
    pub async fn start(&self, task_id: TaskId) -> CoreResult<()> {
        self.call(|reply| DownloadCommand::Start { task_id, reply })
            .await
    }

    /// 暂停指定任务
    pub async fn pause(&self, task_id: TaskId) -> CoreResult<()> {
        self.call(|reply| DownloadCommand::Pause { task_id, reply })
            .await
    }

    /// 恢复指定任务
    pub async fn resume(&self, task_id: TaskId) -> CoreResult<()> {
        self.call(|reply| DownloadCommand::Resume { task_id, reply })
            .await
    }

    /// 重试失败的任务
    pub async fn retry(&self, task_id: TaskId) -> CoreResult<()> {
        self.call(|reply| DownloadCommand::Retry { task_id, reply })
            .await
    }

    /// 移除任务并清理资源（彻底擦除磁盘与数据库记录后确认返回）
    pub async fn remove(&self, task_id: TaskId, delete_file: bool) -> CoreResult<()> {
        self.call(|reply| DownloadCommand::Remove {
            task_id,
            delete_file,
            reply,
        })
        .await
    }

    /// 重新修复补全缺失分片
    pub async fn repair(&self, task_id: TaskId) -> CoreResult<()> {
        self.call(|reply| DownloadCommand::Repair { task_id, reply })
            .await
    }

    /// 更新指定任务的播放流地址（会话过期重新对齐分片）
    pub async fn update_stream_url(
        &self,
        task_id: TaskId,
        stream_url: String,
        referer: Option<String>,
    ) -> CoreResult<()> {
        self.call(|reply| DownloadCommand::UpdateStreamUrl {
            task_id,
            stream_url,
            referer,
            reply,
        })
        .await
    }

    /// 订阅高频进度流（纯 Rust 闭包回调，零框架依赖）
    pub async fn subscribe_progress(&self, callback: ProgressCallback) -> CoreResult<()> {
        self.cmd_tx
            .send(DownloadCommand::SubscribeProgress { callback })
            .await?;
        Ok(())
    }

    /// 取消订阅高频进度流
    pub async fn unsubscribe_progress(&self) -> CoreResult<()> {
        self.cmd_tx
            .send(DownloadCommand::UnsubscribeProgress)
            .await?;
        Ok(())
    }

    /// 动态热调整全局限速带宽 (0 表示不限速)
    pub async fn update_rate_limit(&self, bytes_per_sec: u64) -> CoreResult<()> {
        self.cmd_tx
            .send(DownloadCommand::UpdateRateLimit { bytes_per_sec })
            .await?;
        Ok(())
    }
}

/// 快速启动后台下载管理器主协程，并返回唯一的操作 Handle（确保在 tokio 上下文中使用）
pub fn spawn_download_manager(
    config: Arc<RwLock<AppConfig>>,
    http_client: Arc<RwLock<HttpClient>>,
    db: Arc<Database>,
) -> DownloadManagerHandle {
    let (cmd_tx, cmd_rx) = mpsc::channel(64);
    let (event_tx, _) = broadcast::channel(64);
    let mut manager =
        DownloadManager::new(cmd_rx, event_tx.clone(), config, http_client, db);
    tokio::spawn(async move {
        manager.run().await;
    });
    DownloadManagerHandle::new(cmd_tx, event_tx)
}

// 内部下载任务调度器（纯私有状态机，不对外部暴露，无 Tauri 框架依赖）
struct DownloadManager {
    cmd_rx: mpsc::Receiver<DownloadCommand>, // 从外部接收命令
    status_tx: mpsc::Sender<EngineEvent>,
    status_rx: mpsc::Receiver<EngineEvent>, // 接收 engine 的下载状态与失效事件
    event_tx: broadcast::Sender<TaskEvent>,          // 向外部广播领域事件
    progress_hub: ProgressHub,                       // 共享进度直报管道

    engines: HashMap<TaskId, mpsc::Sender<EngineCommand>>, // 往具体的 engine 发送命令
    pending_queue: VecDeque<TaskId>,                       // 待下载任务排队队列
    task_limits: Arc<Semaphore>,                           // 同时可以进行的下载任务数量
    throttler: Arc<RateThrottler>,                         // 全局速率限流器
    config: Arc<RwLock<AppConfig>>,
    http_client: Arc<RwLock<HttpClient>>,
    db: Arc<Database>,
}

impl DownloadManager {
    fn new(
        cmd_rx: mpsc::Receiver<DownloadCommand>,
        event_tx: broadcast::Sender<TaskEvent>,
        config: Arc<RwLock<AppConfig>>,
        http_client: Arc<RwLock<HttpClient>>,
        db: Arc<Database>,
    ) -> Self {
        let (status_tx, status_rx) = mpsc::channel(32);
        let initial_speed = config.read().max_download_speed;
        let task_limits = Arc::new(Semaphore::new(config.read().max_concurrent_tasks));
        let throttler = Arc::new(RateThrottler::new(initial_speed));

        Self {
            cmd_rx,
            status_tx,
            status_rx,
            event_tx,
            progress_hub: Arc::new(parking_lot::RwLock::new(None)),
            engines: HashMap::new(),
            pending_queue: VecDeque::new(),
            task_limits,
            throttler,
            config,
            http_client,
            db,
        }
    }

    fn spawn_engine(&mut self, record: TaskRecord) -> mpsc::Sender<EngineCommand> {
        let (cmd_tx, cmd_rx) = mpsc::channel(1);
        let task_id = record.id;
        self.engines.insert(task_id, cmd_tx.clone());

        let is_m3u8_stream = is_m3u8(&record.stream_url);
        let preferred_quality = self.config.read().preferred_quality;
        let ctx = EngineContext {
            task: record,
            client: self.http_client.read().clone(),
            progress_hub: self.progress_hub.clone(),
            preferred_quality,
            throttler: self.throttler.clone(),
            db: self.db.clone(),
        };

        if is_m3u8_stream {
            HlsEngine::spawn(ctx, cmd_rx, self.status_tx.clone());
        } else {
            HttpEngine::spawn(ctx, cmd_rx, self.status_tx.clone());
        }

        cmd_tx
    }

    async fn run(&mut self) {
        // === 启动期状态水合 / 预热 ===
        // 恢复所有持久化 tasks 的引擎实例，生命周期伴随终身，统一由 Engine 处理各项操作
        // 数据库在 init() 中已将异常终止/未完成态（running / pending）自愈重置为 paused
        match self.db.get_all_tasks() {
            Ok(all_tasks) => {
                let count = all_tasks.len();
                for record in all_tasks {
                    self.spawn_engine(record);
                }
                if count > 0 {
                    log::info!("DownloadManager: hydrated {} tasks from DB", count);
                }
            }
            Err(e) => {
                log::error!("DownloadManager: failed to hydrate tasks from DB: {}", e);
            }
        }

        loop {
            tokio::select! {
                biased; // 外部用户命令优先级最高

                Some(cmd) = self.cmd_rx.recv() => {
                    match cmd {
                        DownloadCommand::Submit {
                            task_id,
                            task,
                            reply,
                        } => {
                            let save_dir = self.config.read().download_dir.clone();
                            let now = current_timestamp();
                            let auto_start = task.auto_start;

                            let record = TaskRecord {
                                id: task_id,
                                title: task.title,
                                stream_url: task.stream_url,
                                referer: task.referer,
                                source_url: task.source_url,
                                save_dir,
                                status: TaskStatus::Pending,
                                completed_segments: None,
                                total_segments: None,
                                downloaded_bytes: 0,
                                total_bytes: None,
                                created_at: now,
                                updated_at: now,
                            };

                            if let Err(e) = self.db.insert_task(&record) {
                                log::error!("Failed to persist new task {} to DB: {}", task_id, e);
                                let _ = reply.send(Err(e.into()));
                                continue;
                            }

                            // 派生并挂载引擎
                            self.spawn_engine(record.clone());

                            // 全局广播新任务初始挂起状态
                            let _ = self.event_tx.send(TaskEvent::StatusChanged {
                                task_id,
                                status: TaskStatus::Pending,
                            });

                            // 依据请求决议是否立即排队执行
                            if auto_start {
                                self.pending_queue.push_back(task_id);
                                self.schedule().await;
                            }

                            let _ = reply.send(Ok(record));
                        }

                        DownloadCommand::Start { task_id, reply } => {
                            if !self.engines.contains_key(&task_id) {
                                let _ = reply
                                    .send(Err(CoreError::Other(anyhow::anyhow!("Task not found"))));
                                continue;
                            }
                            self.pending_queue.retain(|id| *id != task_id);
                            self.pending_queue.push_front(task_id);
                            self.schedule().await;
                            let _ = reply.send(Ok(()));
                        }

                        DownloadCommand::Pause { task_id, reply } => {
                            // 移出未执行的排队队列
                            self.pending_queue.retain(|id| *id != task_id);
                            if let Some(cmd_tx) = self.engines.get(&task_id) {
                                let _ = cmd_tx.send(EngineCommand::Pause).await;
                                let _ = reply.send(Ok(()));
                            } else {
                                let _ = reply
                                    .send(Err(CoreError::Other(anyhow::anyhow!("Task not found"))));
                            }
                        }

                        DownloadCommand::Resume { task_id, reply } => {
                            if !self.engines.contains_key(&task_id) {
                                let _ = reply
                                    .send(Err(CoreError::Other(anyhow::anyhow!("Task not found"))));
                                continue;
                            }
                            self.pending_queue.retain(|id| *id != task_id);
                            self.pending_queue.push_front(task_id);
                            self.schedule().await;
                            let _ = reply.send(Ok(()));
                        }

                        DownloadCommand::Retry { task_id, reply } => {
                            if !self.engines.contains_key(&task_id) {
                                let _ = reply
                                    .send(Err(CoreError::Other(anyhow::anyhow!("Task not found"))));
                                continue;
                            }
                            // 重置失败任务为 Pending 状态
                            if let Err(e) = self.db.update_status(&task_id, TaskStatus::Pending) {
                                log::error!(
                                    "Failed to reset retry task {} status in DB: {}",
                                    task_id,
                                    e
                                );
                            }
                            let _ = self.event_tx.send(TaskEvent::StatusChanged {
                                task_id,
                                status: TaskStatus::Pending,
                            });

                            self.pending_queue.retain(|id| *id != task_id);
                            self.pending_queue.push_front(task_id);
                            self.schedule().await;
                            let _ = reply.send(Ok(()));
                        }

                        DownloadCommand::Remove {
                            task_id,
                            delete_file,
                            reply,
                        } => {
                            // 1. 从调度等待队列中移出
                            self.pending_queue.retain(|id| *id != task_id);

                            // 2. 停止底层执行引擎并从管理映射中移除：下发 delete_file 参数，由 Engine 统一负责临时分片与合并文件的清理
                            if let Some(cmd_tx) = self.engines.remove(&task_id) {
                                let _ = cmd_tx.send(EngineCommand::Stop { delete_file }).await;
                            }

                            // 3. 数据库持久化记录物理擦除
                            let res = self.db.delete_task(&task_id).map_err(Into::into);
                            let _ = reply.send(res);
                        }

                        DownloadCommand::Repair { task_id, reply } => {
                            if !self.engines.contains_key(&task_id) {
                                let _ = reply
                                    .send(Err(CoreError::Other(anyhow::anyhow!("Task not found"))));
                                continue;
                            }
                            self.pending_queue.retain(|id| *id != task_id);
                            self.pending_queue.push_front(task_id);
                            self.schedule().await;
                            let _ = reply.send(Ok(()));
                        }

                        DownloadCommand::UpdateStreamUrl {
                            task_id,
                            stream_url,
                            referer,
                            reply,
                        } => {
                            if let Err(e) = self.db.update_stream_url(&task_id, &stream_url, referer.as_deref()) {
                                log::error!("Failed to update stream_url in DB for task {}: {}", task_id, e);
                                let _ = reply.send(Err(e.into()));
                                continue;
                            }
                            if let Some(cmd_tx) = self.engines.get(&task_id) {
                                let _ = cmd_tx
                                    .send(EngineCommand::UpdateStreamUrl {
                                        stream_url,
                                        referer,
                                    })
                                    .await;
                            }
                            let _ = reply.send(Ok(()));
                        }

                        DownloadCommand::SubscribeProgress { callback } => {
                            *self.progress_hub.write() = Some(callback);
                        }

                        DownloadCommand::UnsubscribeProgress => {
                            *self.progress_hub.write() = None;
                        }

                        DownloadCommand::UpdateRateLimit { bytes_per_sec } => {
                            self.throttler.set_rate_limit(bytes_per_sec);
                            log::info!(
                                "DownloadManager: updated global rate limit to {} bytes/s",
                                bytes_per_sec
                            );
                        }
                    }
                }

                // 2. 接收来自 engine 的低频状态变迁与失效事件
                Some(event) = self.status_rx.recv() => {
                    match event {
                        EngineEvent::StatusChanged(id, status) => {
                            if let Err(e) = self.db.update_status(&id, status) {
                                log::error!("Failed to update task {} status in DB: {}", id, e);
                            }

                            // 状态变迁全局广播领域事件
                            let _ = self.event_tx.send(TaskEvent::StatusChanged {
                                task_id: id,
                                status,
                            });

                            self.schedule().await;
                        }
                        EngineEvent::StreamExpired(id, status_code) => {
                            log::warn!("DownloadManager: task {} stream expired (HTTP {})", id, status_code);
                            let _ = self.event_tx.send(TaskEvent::StreamExpired {
                                task_id: id,
                                status_code,
                            });
                            self.schedule().await;
                        }
                    }
                }
                else => break,
            }
        }
    }

    async fn schedule(&mut self) {
        while !self.pending_queue.is_empty() {
            let Ok(permit) = self.task_limits.clone().try_acquire_owned() else {
                break;
            };

            let Some(task_id) = self.pending_queue.pop_front() else {
                break;
            };

            if let Some(cmd_tx) = self.engines.get(&task_id) {
                let tx_clone = cmd_tx.clone();
                tokio::spawn(async move {
                    let _ = tx_clone.send(EngineCommand::Start(permit)).await;
                });
            }
        }
    }
}
