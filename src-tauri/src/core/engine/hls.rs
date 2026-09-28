use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::PathBuf,
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
use tokio::{
    io::AsyncWriteExt,
    sync::{mpsc, OwnedSemaphorePermit, Semaphore},
    task::JoinSet,
};
use url::Url;

use super::{DownloadEngine, EngineCommand, EngineContext, EngineEvent, ProgressPayload};
use crate::core::error::{CoreError, CoreResult};
use crate::core::model::TaskStatus;
use crate::core::protocol::hls::{parse_playlist, HlsSegment, ParsedPlaylist};
use crate::core::speed::SpeedTracker;
use crate::core::worker::{Worker, WorkerJob};
use crate::utils::{self, strip_image_disguise};

pub type M3u8Engine = HlsEngine;

const MAX_CONCURRENT_WORKERS: usize = 8; // HLS 切片文件较小，8 并发可最大化吞吐
const MAX_RETRY_TIMES: usize = 3;
const SAMPLING_INTERVAL: u64 = 200; // 采样间隔 200ms

pub struct HlsEngine {
    ctx: EngineContext,
    temp_dir: PathBuf,
    target_path: PathBuf,

    permit: Option<OwnedSemaphorePermit>,
    cmd_rx: mpsc::Receiver<EngineCommand>,
    event_tx: mpsc::Sender<EngineEvent>,

    // 切片与并发控制
    segments: Vec<HlsSegment>,
    pending_segments: VecDeque<usize>,
    segs: BitVec<u8, Lsb0>,
    workers: JoinSet<(usize, CoreResult<()>)>,
    worker_slots: Arc<Semaphore>,
    worker_retry: usize,

    // AES-128 密钥缓存 (Key URI -> Key Bytes)
    key_cache: HashMap<String, [u8; 16]>,

    // 测速与统计 (EWMA 指数加权移动平均工具)
    downloaded_bytes: Arc<AtomicU64>,
    speed_tracker: SpeedTracker,
    last_persisted_percent: u32,
}

impl DownloadEngine for HlsEngine {
    fn spawn(
        ctx: EngineContext,
        cmd_rx: mpsc::Receiver<EngineCommand>,
        event_tx: mpsc::Sender<EngineEvent>,
    ) {
        let mut engine = HlsEngine::new(ctx, cmd_rx, event_tx);
        tokio::spawn(async move { engine.run().await });
    }
}

impl HlsEngine {
    pub fn new(
        ctx: EngineContext,
        cmd_rx: mpsc::Receiver<EngineCommand>,
        event_tx: mpsc::Sender<EngineEvent>,
    ) -> Self {
        let temp_dir = ctx.task.save_dir.join(ctx.task.id.to_string());
        let clean_title = utils::sanitize_file_name(&ctx.task.title);
        let file_name = if clean_title.ends_with(".mp4") {
            clean_title
        } else {
            format!("{}.mp4", clean_title)
        };
        let target_path = ctx.task.save_dir.join(&file_name);

        let initial_percent = if let (Some(completed), Some(total)) =
            (ctx.task.completed_segments, ctx.task.total_segments)
        {
            if total > 0 {
                (completed * 100 / total) as u32
            } else {
                0
            }
        } else {
            0
        };

        Self {
            ctx,
            temp_dir,
            target_path,
            permit: None,
            cmd_rx,
            event_tx,
            segments: Vec::new(),
            pending_segments: VecDeque::new(),
            segs: BitVec::new(),
            workers: JoinSet::new(),
            worker_slots: Arc::new(Semaphore::new(MAX_CONCURRENT_WORKERS)),
            worker_retry: MAX_RETRY_TIMES,
            key_cache: HashMap::new(),
            downloaded_bytes: Arc::new(AtomicU64::new(0)),
            speed_tracker: SpeedTracker::new(),
            last_persisted_percent: initial_percent,
        }
    }

    fn persist_progress(&self) {
        let current_bytes = self.downloaded_bytes.load(Ordering::Relaxed);
        let completed = self.segs.count_ones();
        let total = self.segments.len();
        let _ = self.ctx.db.update_progress(
            &self.ctx.task.id,
            Some(completed),
            Some(total),
            current_bytes,
            None,
        );
    }

    async fn run(&mut self) {
        let mut ticker = tokio::time::interval(Duration::from_millis(SAMPLING_INTERVAL));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                biased;

                // 1. 命令输入：统一在 match (self.ctx.task.status, cmd) 中进行确定性状态转移
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

                            log::info!("Task {}: preparing HLS download...", self.ctx.task.id);
                            if let Err(e) = self.prepare_and_reconcile().await {
                                log::error!("Failed to prepare/reconcile HLS task {}: {}", self.ctx.task.id, e);
                                if let CoreError::HttpStatus(code @ (401 | 403 | 404 | 410)) = e {
                                    log::warn!("HLS Task {}: prepare returned HTTP {}, reporting StreamExpired", self.ctx.task.id, code);
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

                            self.pending_segments = (0..self.segments.len())
                                .filter(|&idx| !self.segs[idx])
                                .collect();

                            if self.pending_segments.is_empty() {
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
                            self.persist_progress();
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
                                "HLS Task {}: received Stop (delete_file={}), engine cleaned up and terminated.",
                                self.ctx.task.id,
                                delete_file
                            );
                            break;
                        },

                        // 任意运行状态收到 UpdateStreamUrl：更新流地址、重拉 M3U8 并重映射切片
                        (_, EngineCommand::UpdateStreamUrl { stream_url, referer }) => {
                            log::info!("HLS Task {}: received UpdateStreamUrl: {}", self.ctx.task.id, stream_url);
                            if let Err(e) = self.remap_stream_with_url(&stream_url, referer.as_deref()).await {
                                log::error!("HLS Task {}: failed to remap stream URL: {}", self.ctx.task.id, e);
                            }
                            if self.ctx.task.status == TaskStatus::Running {
                                self.dispatch_workers();
                            }
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
                            log::warn!("Segment {} download failed: {}", chunk_id, err);
                            self.segs.set(chunk_id, false);
                            if let CoreError::HttpStatus(code @ (401 | 403 | 404 | 410)) = err {
                                log::warn!("HLS Task {}: segment {} returned HTTP {}, reporting StreamExpired", self.ctx.task.id, chunk_id, code);
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

                    // 检查是否全部切片下载完毕
                    if self.pending_segments.is_empty() && self.workers.is_empty() {
                        self.finalize().await;
                    }
                }

                // 3. 测速与进度直报：Running 态下每 200ms 使用 SpeedTracker（EWMA 平滑计算）并直报前端
                _ = ticker.tick(), if self.ctx.task.status == TaskStatus::Running => {
                    let current_bytes = self.downloaded_bytes.load(Ordering::Relaxed);
                    let speed_bps = self.speed_tracker.update(current_bytes, SAMPLING_INTERVAL);
                    let completed = self.segs.count_ones();
                    let total = self.segments.len();

                    // 检查切片完成进度，跨越 1% 阈值按需更新数据库
                    if total > 0 {
                        let current_percent = (completed * 100 / total) as u32;
                        if current_percent > self.last_persisted_percent {
                            self.last_persisted_percent = current_percent;
                            self.persist_progress();
                        }
                    }

                    let payload = ProgressPayload {
                        task_id: self.ctx.task.id,
                        downloaded_bytes: Some(current_bytes),
                        total_bytes: None,
                        completed_segments: Some(completed),
                        total_segments: Some(total),
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

    /// 统一收尾逻辑：聚合切片（剥离伪装头）、FFmpeg MP4 转封装、上报终态事件、清理临时文件并释放并发配额
    async fn finalize(&mut self) {
        log::info!(
            "Task {}: all HLS segments present/downloaded, merging and remuxing...",
            self.ctx.task.id
        );
        self.persist_progress();

        if let Err(e) = self.merge_segments_and_remux().await {
            log::error!("Task {}: merge and remux failed: {}", self.ctx.task.id, e);
            self.ctx.task.status = TaskStatus::Failed;
            _ = self
                .event_tx
                .send(EngineEvent::StatusChanged(self.ctx.task.id, TaskStatus::Failed))
                .await;
        } else if self.segs.all() {
            log::info!(
                "Task {}: all segments merged & remuxed! Cleaning temp directory...",
                self.ctx.task.id
            );
            self.ctx.task.status = TaskStatus::Completed;
            let final_bytes = std::fs::metadata(&self.target_path).map(|m| m.len()).ok();
            let _ = self.ctx.db.update_progress(
                &self.ctx.task.id,
                Some(self.segments.len()),
                Some(self.segments.len()),
                final_bytes.unwrap_or_else(|| self.downloaded_bytes.load(Ordering::Relaxed)),
                final_bytes,
            );
            if self.temp_dir.exists() {
                let _ = tokio::fs::remove_dir_all(&self.temp_dir).await;
            }
            _ = self
                .event_tx
                .send(EngineEvent::StatusChanged(self.ctx.task.id, TaskStatus::Completed))
                .await;
        } else {
            log::warn!(
                "Task {} merged with missing segments: {}/{}. Retaining temp files for repair.",
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

    /// 核心逻辑：按切片序号聚合所有 TS 文件，并在追加写入时自动检测剥离每个分片的伪装图片头（PNG/JPEG等）
    async fn merge_segments_and_remux(&self) -> CoreResult<()> {
        let merged_ts_path = self.temp_dir.join("merged_stream.ts");

        let mut out_file = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&merged_ts_path)
            .await?;

        let mut merged_count = 0usize;

        for idx in 0..self.segments.len() {
            let seg_path = self.temp_dir.join(format!("{:05}.ts", idx));
            if !seg_path.exists() {
                continue;
            }

            let raw_bytes = tokio::fs::read(&seg_path).await?;
            // 剥离可能存在的 PNG/JPEG 伪装头，输出纯净的 TS 数据
            let clean_ts = strip_image_disguise(&raw_bytes);
            out_file.write_all(clean_ts).await?;
            merged_count += 1;
        }

        out_file.flush().await?;
        drop(out_file);

        if merged_count == 0 {
            let _ = tokio::fs::remove_file(&merged_ts_path).await;
            return Err(CoreError::MergeNoValidSegments(merged_ts_path));
        }

        log::info!(
            "Task {}: merged {} segments (all disguise headers stripped) to {:?}, starting remux...",
            self.ctx.task.id,
            merged_count,
            merged_ts_path
        );

        // 调用 remux 进行无损 MP4 转封装
        crate::utils::remux_ts_to_mp4(&merged_ts_path, &self.target_path).await?;

        log::info!(
            "Task {}: HLS stream successfully remuxed to {:?}",
            self.ctx.task.id,
            self.target_path
        );

        Ok(())
    }

    fn dispatch_workers(&mut self) {
        while let Ok(permit) = self.worker_slots.clone().try_acquire_owned() {
            if let Some(seg_id) = self.pending_segments.pop_front() {
                let seg = &self.segments[seg_id];

                let mut aes_key = None;
                let mut aes_iv = None;

                if let Some(ref key_info) = seg.key {
                    if let Some(k) = self.key_cache.get(&key_info.uri) {
                        aes_key = Some(*k);
                    }
                    if let Some(ref iv_bytes) = key_info.iv {
                        if iv_bytes.len() == 16 {
                            let mut iv = [0u8; 16];
                            iv.copy_from_slice(iv_bytes);
                            aes_iv = Some(iv);
                        }
                    } else {
                        // RFC 8216 Section 5.2: sequence number as IV
                        let mut iv = [0u8; 16];
                        iv[8..16].copy_from_slice(&(seg_id as u64).to_be_bytes());
                        aes_iv = Some(iv);
                    }
                }

                let job = WorkerJob::builder(
                    seg_id,
                    seg.url.clone(),
                    self.temp_dir.join(format!("{:05}.ts", seg_id)),
                    self.ctx.client.clone(),
                    self.downloaded_bytes.clone(),
                    self.ctx.throttler.clone(),
                )
                .referer(self.ctx.task.referer.clone())
                .retry_times(self.worker_retry)
                .aes(aes_key, aes_iv)
                .build();

                self.workers.spawn(async move {
                    let _permit = permit;
                    let res = Worker::execute(job).await;
                    (seg_id, res)
                });
            } else {
                break;
            }
        }
    }

    async fn prepare_and_reconcile(&mut self) -> CoreResult<()> {
        tokio::fs::create_dir_all(&self.temp_dir).await?;

        // 1. 若内存中尚无切片信息，优先尝试从磁盘缓存 segments.json 读取
        let segments_cache_path = self.temp_dir.join("segments.json");
        if self.segments.is_empty() && segments_cache_path.exists() {
            if let Ok(content) = tokio::fs::read_to_string(&segments_cache_path).await {
                if let Ok(cached_segments) = serde_json::from_str::<Vec<HlsSegment>>(&content) {
                    if !cached_segments.is_empty() {
                        self.segments = cached_segments;
                    }
                }
            }
        }

        // 2. 若仍无切片，拉取并解析 M3U8
        if self.segments.is_empty() {
            let segs = self.fetch_and_parse_m3u8().await?;
            self.update_segments_cache(segs).await?;
        }

        // 3. 对齐本地已落盘的切片（断点续传），并保证 AES 密钥就绪
        self.reconcile_disk_segments().await;
        self.ensure_aes_keys().await;

        Ok(())
    }

    /// 拉取并解析 M3U8（支持 Master 播放列表自适应画质选择）
    async fn fetch_and_parse_m3u8(&self) -> CoreResult<Vec<HlsSegment>> {
        let mut req = self.ctx.client.get(&self.ctx.task.stream_url);
        if let Some(ref referer) = self.ctx.task.referer {
            if !self.ctx.task.stream_url.contains("googleusercontent.com") && !self.ctx.task.stream_url.contains("googlevideo.com") {
                req = req.header("Referer", referer);
            }
        }
        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(CoreError::HttpStatus(resp.status().as_u16()));
        }

        let m3u8_bytes = resp.bytes().await?;
        let base_url = Url::parse(&self.ctx.task.stream_url)?;
        let parsed = parse_playlist(&base_url, &m3u8_bytes, self.ctx.preferred_quality)?;

        let segments = match parsed {
            ParsedPlaylist::Media(segs) => segs,
            ParsedPlaylist::Master { sub_m3u8_url, .. } => {
                log::info!(
                    "Task {}: Master playlist detected, fetching variant: {}",
                    self.ctx.task.id,
                    sub_m3u8_url
                );
                let mut sub_req = self.ctx.client.get(sub_m3u8_url.as_str());
                if let Some(ref referer) = self.ctx.task.referer {
                    if !sub_m3u8_url.as_str().contains("googleusercontent.com") && !sub_m3u8_url.as_str().contains("googlevideo.com") {
                        sub_req = sub_req.header("Referer", referer);
                    }
                }
                let sub_resp = sub_req.send().await?;
                if !sub_resp.status().is_success() {
                    return Err(CoreError::HttpStatus(sub_resp.status().as_u16()));
                }
                let sub_bytes = sub_resp.bytes().await?;
                let sub_parsed =
                    parse_playlist(&sub_m3u8_url, &sub_bytes, self.ctx.preferred_quality)?;
                match sub_parsed {
                    ParsedPlaylist::Media(segs) => segs,
                    ParsedPlaylist::Master { .. } => {
                        return Err(CoreError::NestedMasterPlaylist);
                    }
                }
            }
        };

        Ok(segments)
    }

    /// 更新切片列表并在磁盘持久化 segments.json
    async fn update_segments_cache(&mut self, new_segments: Vec<HlsSegment>) -> CoreResult<()> {
        let segments_cache_path = self.temp_dir.join("segments.json");
        if let Ok(json) = serde_json::to_string_pretty(&new_segments) {
            let _ = tokio::fs::write(&segments_cache_path, json).await;
        }
        self.segments = new_segments;
        Ok(())
    }

    /// 使用新的 stream_url 重新映射切片（无损断点续传）
    async fn remap_stream_with_url(
        &mut self,
        stream_url: &str,
        referer: Option<&str>,
    ) -> CoreResult<()> {
        log::info!(
            "Task {}: remapping stream with new URL: {} (referer={:?})",
            self.ctx.task.id,
            stream_url,
            referer
        );
        self.ctx.task.stream_url = stream_url.to_string();
        self.ctx.task.referer = referer.map(|s| s.to_string());
        let _ = self
            .ctx
            .db
            .update_stream_url(&self.ctx.task.id, stream_url, referer);

        let segs = self.fetch_and_parse_m3u8().await?;
        self.update_segments_cache(segs).await?;
        self.reconcile_disk_segments().await;
        self.ensure_aes_keys().await;

        self.pending_segments = (0..self.segments.len())
            .filter(|&idx| !self.segs[idx])
            .collect();

        Ok(())
    }


    /// 预热与缓存 AES-128 解密密钥
    async fn ensure_aes_keys(&mut self) {
        let unique_keys: HashSet<String> = self
            .segments
            .iter()
            .filter_map(|s| s.key.as_ref().map(|k| k.uri.clone()))
            .collect();

        for key_uri in unique_keys {
            if self.key_cache.contains_key(&key_uri) {
                continue;
            }
            log::info!("Task {}: fetching AES key: {}", self.ctx.task.id, key_uri);
            let mut req = self.ctx.client.get(&key_uri);
            if let Some(ref referer) = self.ctx.task.referer {
                if !key_uri.contains("googleusercontent.com") && !key_uri.contains("googlevideo.com") {
                    req = req.header("Referer", referer);
                }
            }
            match tokio::time::timeout(Duration::from_secs(8), req.send()).await {
                Ok(Ok(resp)) => {
                    if resp.status().is_success() {
                        if let Ok(key_bytes) = resp.bytes().await {
                            if key_bytes.len() == 16 {
                                let mut key_arr = [0u8; 16];
                                key_arr.copy_from_slice(&key_bytes);
                                self.key_cache.insert(key_uri, key_arr);
                                log::info!(
                                    "Task {}: AES key acquired successfully",
                                    self.ctx.task.id
                                );
                            } else {
                                log::error!(
                                    "Task {}: invalid AES key length: {} (expected 16)",
                                    self.ctx.task.id,
                                    key_bytes.len()
                                );
                            }
                        }
                    } else {
                        log::warn!(
                            "Task {}: AES key fetch HTTP status: {}",
                            self.ctx.task.id,
                            resp.status()
                        );
                    }
                }
                Ok(Err(e)) => {
                    log::warn!("Task {}: failed to fetch AES key: {}", self.ctx.task.id, e);
                }
                Err(_) => {
                    log::warn!(
                        "Task {}: timeout fetching AES key from {}",
                        self.ctx.task.id,
                        key_uri
                    );
                }
            }
        }
    }

    /// 扫描临时目录已落盘且大小非 0 的切片文件（断点对齐）
    async fn reconcile_disk_segments(&mut self) {
        let total_segments = self.segments.len();
        self.segs = bitvec![u8, Lsb0; 0; total_segments];

        let mut recovered = 0usize;
        if let Ok(mut entries) = tokio::fs::read_dir(&self.temp_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let file_name = entry.file_name();
                let name = file_name.to_string_lossy();
                if name.ends_with(".ts") && !name.starts_with("merged_") {
                    let num_part = &name[..name.len() - 3];
                    if let Ok(idx) = num_part.parse::<usize>() {
                        if idx < total_segments {
                            if let Ok(meta) = entry.metadata().await {
                                if meta.len() > 0 {
                                    self.segs.set(idx, true);
                                    recovered += 1;
                                }
                            }
                        }
                    }
                }
            }
        }

        log::info!(
            "Task {}: reconciled {}/{} segments from disk",
            self.ctx.task.id,
            recovered,
            total_segments
        );

        self.last_persisted_percent = if total_segments > 0 {
            (recovered * 100 / total_segments) as u32
        } else {
            0
        };
        self.persist_progress();
    }
}
