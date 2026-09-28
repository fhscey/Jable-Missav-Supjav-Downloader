use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use bitvec::{
    order::Lsb0,
    prelude::{bitvec, BitVec},
};
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::{mpsc, OwnedSemaphorePermit, Semaphore},
    task::JoinSet,
};

use super::{DownloadEngine, EngineCommand, EngineContext, EngineEvent, ProgressPayload};
use crate::core::error::{CoreError, CoreResult};
use crate::core::model::TaskStatus;
use crate::core::protocol::http::probe_http_stream;
use crate::core::speed::SpeedTracker;
use crate::core::worker::{Worker, WorkerJob};
use crate::utils;

const MAX_CONCURRENT_WORKERS: usize = 4; // 常规单文件 HTTP 下载，4 并发最为均衡稳定
const DEFAULT_CHUNK_SIZE: u64 = 4 * 1024 * 1024; // 默认 4MB 一个分块
const MAX_RETRY_TIMES: usize = 3;
const SAMPLING_INTERVAL: u64 = 200; // 采样间隔 200ms

#[derive(Debug, Clone)]
pub struct HttpChunk {
    pub chunk_id: usize,
    pub start: u64,
    pub end: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpMetaCache {
    pub total_size: Option<u64>,
    pub accept_ranges: bool,
    pub chunk_size: u64,
    pub total_chunks: usize,
    pub content_type: Option<String>,
}

pub struct HttpEngine {
    ctx: EngineContext,
    temp_dir: PathBuf,
    target_path: PathBuf,

    permit: Option<OwnedSemaphorePermit>,
    cmd_rx: mpsc::Receiver<EngineCommand>,
    event_tx: mpsc::Sender<EngineEvent>,

    // 分块与并发控制
    total_size: Option<u64>,
    accept_ranges: bool,
    chunks: Vec<HttpChunk>,
    pending_chunks: VecDeque<usize>,
    segs: BitVec<u8, Lsb0>,
    workers: JoinSet<(usize, CoreResult<()>)>,
    worker_slots: Arc<Semaphore>,
    worker_retry: usize,

    // 测速与统计 (EWMA 指数加权移动平均工具)
    downloaded_bytes: Arc<AtomicU64>,
    speed_tracker: SpeedTracker,
    last_persisted_percent: u32,
}

impl DownloadEngine for HttpEngine {
    fn spawn(
        ctx: EngineContext,
        cmd_rx: mpsc::Receiver<EngineCommand>,
        event_tx: mpsc::Sender<EngineEvent>,
    ) {
        let mut engine = HttpEngine::new(ctx, cmd_rx, event_tx);
        tokio::spawn(async move { engine.run().await });
    }
}

impl HttpEngine {
    pub fn new(
        ctx: EngineContext,
        cmd_rx: mpsc::Receiver<EngineCommand>,
        event_tx: mpsc::Sender<EngineEvent>,
    ) -> Self {
        let temp_dir = ctx.task.save_dir.join(ctx.task.id.to_string());
        let clean_title = utils::sanitize_file_name(&ctx.task.title);
        let file_name = if clean_title.contains('.') {
            clean_title
        } else if let Ok(parsed) = url::Url::parse(&ctx.task.stream_url) {
            if let Some(ext) = std::path::Path::new(parsed.path())
                .extension()
                .and_then(|e| e.to_str())
            {
                format!("{}.{}", clean_title, ext)
            } else {
                format!("{}.mp4", clean_title)
            }
        } else {
            format!("{}.mp4", clean_title)
        };
        let target_path = ctx.task.save_dir.join(&file_name);
        let initial_percent = ctx
            .task
            .total_bytes
            .and_then(|total| (ctx.task.downloaded_bytes * 100).checked_div(total))
            .unwrap_or(0) as u32;

        Self {
            ctx,
            temp_dir,
            target_path,
            permit: None,
            cmd_rx,
            event_tx,
            total_size: None,
            accept_ranges: false,
            chunks: Vec::new(),
            pending_chunks: VecDeque::new(),
            segs: BitVec::new(),
            workers: JoinSet::new(),
            worker_slots: Arc::new(Semaphore::new(MAX_CONCURRENT_WORKERS)),
            worker_retry: MAX_RETRY_TIMES,
            downloaded_bytes: Arc::new(AtomicU64::new(0)),
            speed_tracker: SpeedTracker::new(),
            last_persisted_percent: initial_percent,
        }
    }

    fn persist_progress(&self, total_bytes: Option<u64>) {
        let current_bytes = self.downloaded_bytes.load(Ordering::Relaxed);
        let _ = self.ctx.db.update_progress(
            &self.ctx.task.id,
            None,
            None,
            current_bytes,
            self.total_size.or(total_bytes),
        );
    }

    async fn run(&mut self) {
        let mut ticker = tokio::time::interval(Duration::from_millis(SAMPLING_INTERVAL));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                biased;

                // 1. 命令输入：统一在 match (self.ctx.task.status, cmd) 中进行确定性状态转移
                //
                // 【状态机设计原则说明】：
                // 1. Engine 侧保持最纯净的 3 个底层原语：Start(permit), Pause, Stop。
                // 2. 状态驱动契约：
                //    - Pending / Paused / Failed / Incomplete + Start(permit) ->
                //      统一注入并发 Permit，状态置为 Running，校验并拉齐分片断点。
                //      若有缺失分块则继续并发下载；全部就绪则触发本地组装合并并转 Completed。
                //    - Running / Pending + Pause -> 释放 permit 归还全局并发配额，中止分块 workers，进入 Paused。
                //    - Any + Stop -> 释放 permit，中止 workers，退出事件循环（外部物理删除由 Manager 统一处理）。
                Some(cmd) = self.cmd_rx.recv() => {
                    match (self.ctx.task.status, cmd) {
                        (
                            TaskStatus::Pending
                            | TaskStatus::Paused
                            | TaskStatus::Failed
                            | TaskStatus::Incomplete,
                            EngineCommand::Start(permit)
                        ) => {
                            self.permit = Some(permit);
                            self.ctx.task.status = TaskStatus::Running;
                            _ = self.event_tx.send(EngineEvent::StatusChanged(self.ctx.task.id, TaskStatus::Running)).await;

                            log::info!("Task {}: preparing HTTP download...", self.ctx.task.id);
                            if let Err(e) = self.prepare_and_reconcile().await {
                                log::error!("Failed to prepare/reconcile HTTP task {}: {}", self.ctx.task.id, e);
                                if let CoreError::HttpStatus(code @ (401 | 403 | 404 | 410)) = e {
                                    self.ctx.task.status = TaskStatus::Paused;
                                    self.permit.take();
                                    _ = self.event_tx.send(EngineEvent::StreamExpired(self.ctx.task.id, code)).await;
                                    continue;
                                }
                                self.ctx.task.status = TaskStatus::Failed;
                                _ = self.event_tx.send(EngineEvent::StatusChanged(self.ctx.task.id, TaskStatus::Failed)).await;
                                self.permit.take();
                                continue;
                            }

                            self.pending_chunks = (0..self.chunks.len())
                                .filter(|&idx| !self.segs[idx])
                                .collect();

                            if self.pending_chunks.is_empty() {
                                self.finalize().await;
                            } else {
                                self.dispatch_workers();
                            }
                        },

                        (TaskStatus::Pending, EngineCommand::Pause) => {
                            self.ctx.task.status = TaskStatus::Paused;
                            _ = self.event_tx.send(EngineEvent::StatusChanged(self.ctx.task.id, TaskStatus::Paused)).await;
                        },

                        (TaskStatus::Running, EngineCommand::Pause) => {
                            self.persist_progress(None);
                            self.ctx.task.status = TaskStatus::Paused;
                            self.permit.take();
                            self.workers.abort_all();
                            while self.workers.join_next().await.is_some() {}
                            self.speed_tracker.reset(self.downloaded_bytes.load(Ordering::Relaxed));
                            _ = self.event_tx.send(EngineEvent::StatusChanged(self.ctx.task.id, TaskStatus::Paused)).await;
                        },

                        // 任意运行状态收到 Stop：释放全局资源，中止 workers，清理临时目录，并可选删除最终文件
                        (_, EngineCommand::Stop { delete_file }) => {
                            self.permit.take();
                            self.workers.abort_all();
                            if self.temp_dir.exists() {
                                let _ = tokio::fs::remove_dir_all(&self.temp_dir).await;
                            }
                            if delete_file && self.target_path.exists() {
                                let _ = tokio::fs::remove_file(&self.target_path).await;
                            }
                            log::info!(
                                "HTTP Task {}: received Stop (delete_file={}), engine cleaned up and terminated.",
                                self.ctx.task.id,
                                delete_file
                            );
                            break;
                        },

                        // 任意运行状态收到 UpdateStreamUrl：更新流地址与 Referer
                        (_, EngineCommand::UpdateStreamUrl { stream_url, referer }) => {
                            log::info!("HTTP Task {}: received UpdateStreamUrl: {}", self.ctx.task.id, stream_url);
                            self.ctx.task.stream_url = stream_url;
                            self.ctx.task.referer = referer;
                        },

                        (status, cmd) => {
                            log::warn!("Ignored transition: current_status={:?}, cmd={:?}", status, cmd);
                        }
                    }
                }

                // 2. 状态驱动：Running 态下驱动 worker 产出
                Some(res) = self.workers.join_next(), if self.ctx.task.status == TaskStatus::Running => {
                    match res {
                        Ok((chunk_id, Ok(()))) => {
                            self.segs.set(chunk_id, true);
                        }
                        Ok((chunk_id, Err(err))) => {
                            log::warn!("Chunk {} download failed: {}", chunk_id, err);
                            self.segs.set(chunk_id, false);
                            if let CoreError::HttpStatus(code @ (401 | 403 | 404 | 410)) = err {
                                log::warn!("HTTP Task {}: chunk {} returned HTTP {}, reporting StreamExpired", self.ctx.task.id, chunk_id, code);
                                self.permit.take();
                                self.workers.abort_all();
                                while self.workers.join_next().await.is_some() {}
                                self.ctx.task.status = TaskStatus::Paused;
                                _ = self.event_tx.send(EngineEvent::StreamExpired(self.ctx.task.id, code)).await;
                            }
                        }
                        Err(_abort_err) => {}
                    }

                    self.dispatch_workers();

                    // 检查是否全部下载完毕
                    if self.pending_chunks.is_empty() && self.workers.is_empty() {
                        self.finalize().await;
                    }
                }

                // 3. 测速与进度直报：Running 态下每 200ms 使用 SpeedTracker（EWMA 平滑计算）并直报前端
                _ = ticker.tick(), if self.ctx.task.status == TaskStatus::Running => {
                    let current_bytes = self.downloaded_bytes.load(Ordering::Relaxed);
                    let speed_bps = self.speed_tracker.update(current_bytes, SAMPLING_INTERVAL);

                    // 检查是否跨越 1% 阈值，按需落盘数据库
                    if let Some(total) = self.total_size {
                        if total > 0 {
                            let current_percent = (current_bytes * 100 / total) as u32;
                            if current_percent > self.last_persisted_percent {
                                self.last_persisted_percent = current_percent;
                                self.persist_progress(None);
                            }
                        }
                    }

                    let payload = ProgressPayload {
                        task_id: self.ctx.task.id,
                        downloaded_bytes: Some(current_bytes),
                        total_bytes: self.total_size,
                        completed_segments: None,
                        total_segments: None,
                        speed_bps,
                    };
                    if let Some(ref cb) = *self.ctx.progress_hub.read() {
                        cb(payload);
                    }
                }

                else => break,
            }
        }
    }

    /// 统一收尾逻辑：合并分块文件、完整性校验、上报终态事件、清理临时文件并释放并发配额
    async fn finalize(&mut self) {
        log::info!(
            "Task {}: all chunks present/downloaded, finalizing...",
            self.ctx.task.id
        );
        self.persist_progress(None);
        if let Err(e) = self.merge_chunks().await {
            log::error!("Task {}: merge failed: {}", self.ctx.task.id, e);
            self.ctx.task.status = TaskStatus::Failed;
            _ = self
                .event_tx
                .send(EngineEvent::StatusChanged(self.ctx.task.id, TaskStatus::Failed))
                .await;
        } else if self.segs.all() {
            log::info!(
                "Task {}: all chunks downloaded and merged! Cleaning temp files...",
                self.ctx.task.id
            );
            self.ctx.task.status = TaskStatus::Completed;
            let final_bytes = std::fs::metadata(&self.target_path)
                .map(|m| m.len())
                .ok()
                .or(self.total_size);
            self.persist_progress(final_bytes);
            if self.temp_dir.exists() {
                let _ = tokio::fs::remove_dir_all(&self.temp_dir).await;
            }
            _ = self
                .event_tx
                .send(EngineEvent::StatusChanged(self.ctx.task.id, TaskStatus::Completed))
                .await;
        } else {
            log::warn!(
                "Task {} merged with missing chunks: {}/{}. Retaining temp files for repair.",
                self.ctx.task.id,
                self.segs.count_ones(),
                self.segs.len()
            );
            self.ctx.task.status = TaskStatus::Incomplete;
            _ = self
                .event_tx
                .send(EngineEvent::StatusChanged(self.ctx.task.id, TaskStatus::Incomplete))
                .await;
        }
        self.permit.take();
    }

    fn dispatch_workers(&mut self) {
        while let Ok(permit) = self.worker_slots.clone().try_acquire_owned() {
            if let Some(chunk_id) = self.pending_chunks.pop_front() {
                let chunk = &self.chunks[chunk_id];
                let range = if self.accept_ranges {
                    Some((chunk.start, chunk.end))
                } else {
                    None
                };

                let job = WorkerJob::builder(
                    chunk_id,
                    self.ctx.task.stream_url.clone(),
                    self.temp_dir.join(format!("{:05}.chunk", chunk_id)),
                    self.ctx.client.clone(),
                    self.downloaded_bytes.clone(),
                    self.ctx.throttler.clone(),
                )
                .referer(self.ctx.task.referer.clone())
                .retry_times(self.worker_retry)
                .range(range)
                .build();

                self.workers.spawn(async move {
                    let _permit = permit;
                    let res = Worker::execute(job).await;
                    (chunk_id, res)
                });
            } else {
                break;
            }
        }
    }

    async fn load_or_probe_meta(&self, meta_path: &Path) -> CoreResult<HttpMetaCache> {
        if meta_path.exists() {
            if let Ok(content) = tokio::fs::read_to_string(meta_path).await {
                if let Ok(meta) = serde_json::from_str::<HttpMetaCache>(&content) {
                    log::info!(
                        "Task {}: loaded meta from cache (total_size={:?}, accept_ranges={}, chunks={})",
                        self.ctx.task.id,
                        meta.total_size,
                        meta.accept_ranges,
                        meta.total_chunks
                    );
                    return Ok(meta);
                }
            }
        }

        // 发起 HEAD 请求探测资源
        let probe = probe_http_stream(
            &self.ctx.client,
            &self.ctx.task.stream_url,
            self.ctx.task.referer.as_deref(),
        )
        .await?;

        let total_size = probe.content_length;
        let accept_ranges = probe.accept_ranges && total_size.is_some();
        let chunk_size = DEFAULT_CHUNK_SIZE;
        let total_chunks = if accept_ranges {
            let total = total_size.unwrap();
            (total.div_ceil(chunk_size) as usize).max(1)
        } else {
            1
        };

        let meta = HttpMetaCache {
            total_size,
            accept_ranges,
            chunk_size,
            total_chunks,
            content_type: probe.content_type,
        };

        if let Ok(json) = serde_json::to_string_pretty(&meta) {
            let _ = tokio::fs::write(meta_path, json).await;
        }

        Ok(meta)
    }

    async fn prepare_and_reconcile(&mut self) -> CoreResult<()> {
        if !self.chunks.is_empty() {
            return Ok(());
        }

        tokio::fs::create_dir_all(&self.temp_dir).await?;

        let meta_path = self.temp_dir.join("meta.json");
        let meta = self.load_or_probe_meta(&meta_path).await?;

        self.total_size = meta.total_size;
        self.accept_ranges = meta.accept_ranges;

        let total_chunks = meta.total_chunks.max(1);
        let mut chunks = Vec::with_capacity(total_chunks);

        if meta.accept_ranges && meta.total_size.is_some() {
            let total = meta.total_size.unwrap();
            for i in 0..total_chunks {
                let start = i as u64 * meta.chunk_size;
                let end = ((i as u64 + 1) * meta.chunk_size).min(total) - 1;
                chunks.push(HttpChunk {
                    chunk_id: i,
                    start,
                    end,
                });
            }
        } else {
            // 不支持 Range 时退化为单分块流式下载
            chunks.push(HttpChunk {
                chunk_id: 0,
                start: 0,
                end: meta.total_size.unwrap_or(0),
            });
        }

        self.segs = bitvec![u8, Lsb0; 0; total_chunks];
        self.chunks = chunks;

        // 断点恢复：扫描临时目录已落盘且大小吻合的分块
        for (idx, chunk) in self.chunks.iter().enumerate() {
            let chunk_path = self.temp_dir.join(format!("{:05}.chunk", idx));
            if let Ok(file_meta) = tokio::fs::metadata(&chunk_path).await {
                if self.accept_ranges {
                    let expected_size = chunk.end - chunk.start + 1;
                    if file_meta.len() == expected_size {
                        self.segs.set(idx, true);
                    }
                } else if file_meta.len() > 0 {
                    if let Some(total) = self.total_size {
                        if file_meta.len() == total {
                            self.segs.set(idx, true);
                        }
                    }
                }
            }
        }

        let recovered = self.segs.count_ones();
        if recovered > 0 {
            log::info!(
                "Task {}: recovered {}/{} chunks from disk",
                self.ctx.task.id,
                recovered,
                total_chunks
            );
        }

        self.last_persisted_percent = if let Some(total) = self.total_size {
            if total > 0 {
                let current_bytes = self.downloaded_bytes.load(Ordering::Relaxed);
                (current_bytes * 100 / total) as u32
            } else {
                0
            }
        } else {
            0
        };
        self.persist_progress(None);

        Ok(())
    }

    async fn merge_chunks(&self) -> CoreResult<PathBuf> {
        let merging_temp_path = self.temp_dir.join("merging_output.tmp");

        let mut out_file = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&merging_temp_path)
            .await?;

        let mut buffer = vec![0u8; 1024 * 1024]; // 1MB 缓冲
        let mut merged_count = 0usize;

        for idx in 0..self.chunks.len() {
            let chunk_path = self.temp_dir.join(format!("{:05}.chunk", idx));
            if !chunk_path.exists() {
                continue;
            }

            let mut chunk_file = tokio::fs::File::open(&chunk_path).await?;
            loop {
                let bytes_read = chunk_file.read(&mut buffer).await?;
                if bytes_read == 0 {
                    break;
                }
                out_file.write_all(&buffer[..bytes_read]).await?;
            }
            merged_count += 1;
        }

        out_file.flush().await?;
        drop(out_file);

        if merged_count == 0 {
            let _ = tokio::fs::remove_file(&merging_temp_path).await;
            return Err(CoreError::MergeNoValidSegments(merging_temp_path));
        }

        utils::move_or_copy_file(&merging_temp_path, &self.target_path).await?;

        log::info!(
            "Task {}: HTTP file successfully merged to {:?}",
            self.ctx.task.id,
            self.target_path
        );

        Ok(self.target_path.clone())
    }
}
