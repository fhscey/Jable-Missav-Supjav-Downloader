use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use tokio::io::AsyncWriteExt;

use crate::core::error::{CoreError, CoreResult};
use crate::core::network::HttpClient;
use crate::core::protocol::hls::decrypt_aes128;
use crate::core::throttler::RateThrottler;
use crate::utils;

pub struct WorkerJob {
    pub chunk_id: usize,                  // m3u8 的 seg id 或者 http chunk id
    pub url: String,                      // 具体的下载地址
    pub save_path: PathBuf,               // 临时存储路径
    pub client: HttpClient,               // http 客户端
    pub referer: Option<String>,          // Referer 头
    pub retry_times: usize,               // 重试次数
    pub downloaded_bytes: Arc<AtomicU64>, // 这个是整个文件的已下载数，每个 worker 往上累加
    pub range: Option<(u64, u64)>,        // 可选分块 Range (start_byte, end_byte)
    pub throttler: Arc<RateThrottler>,    // 全局速率限流器
    pub aes_key: Option<[u8; 16]>,        // HLS AES-128 解密密钥（如有）
    pub aes_iv: Option<[u8; 16]>,         // HLS AES-128 初始化向量（如有）
}

impl WorkerJob {
    pub fn builder(
        chunk_id: usize,
        url: impl Into<String>,
        save_path: PathBuf,
        client: HttpClient,
        downloaded_bytes: Arc<AtomicU64>,
        throttler: Arc<RateThrottler>,
    ) -> WorkerJobBuilder {
        WorkerJobBuilder {
            chunk_id,
            url: url.into(),
            save_path,
            client,
            referer: None,
            retry_times: 3,
            downloaded_bytes,
            range: None,
            throttler,
            aes_key: None,
            aes_iv: None,
        }
    }
}

#[derive(Clone)]
pub struct WorkerJobBuilder {
    chunk_id: usize,
    url: String,
    save_path: PathBuf,
    client: HttpClient,
    referer: Option<String>,
    retry_times: usize,
    downloaded_bytes: Arc<AtomicU64>,
    range: Option<(u64, u64)>,
    throttler: Arc<RateThrottler>,
    aes_key: Option<[u8; 16]>,
    aes_iv: Option<[u8; 16]>,
}

impl WorkerJobBuilder {
    pub fn referer(mut self, referer: impl Into<Option<String>>) -> Self {
        self.referer = referer.into();
        self
    }

    pub fn retry_times(mut self, retry_times: usize) -> Self {
        self.retry_times = retry_times;
        self
    }

    pub fn range(mut self, range: impl Into<Option<(u64, u64)>>) -> Self {
        self.range = range.into();
        self
    }

    pub fn aes(mut self, key: Option<[u8; 16]>, iv: Option<[u8; 16]>) -> Self {
        self.aes_key = key;
        self.aes_iv = iv;
        self
    }

    pub fn build(self) -> WorkerJob {
        WorkerJob {
            chunk_id: self.chunk_id,
            url: self.url,
            save_path: self.save_path,
            client: self.client,
            referer: self.referer,
            retry_times: self.retry_times,
            downloaded_bytes: self.downloaded_bytes,
            range: self.range,
            throttler: self.throttler,
            aes_key: self.aes_key,
            aes_iv: self.aes_iv,
        }
    }
}

pub struct Worker;

impl Worker {
    pub async fn execute(job: WorkerJob) -> CoreResult<()> {
        let tmp_path = PathBuf::from(format!("{}.tmp", job.save_path.display()));

        let max_retries = job.retry_times;
        let mut last_err: Option<CoreError> = None;

        for attempt in 0..=max_retries {
            if attempt > 0 {
                let is_rate_limit = matches!(last_err, Some(CoreError::HttpStatus(429)));
                let backoff_ms = if is_rate_limit {
                    1000 * (1 << attempt.min(3))
                } else {
                    500 * (1 << (attempt - 1).min(3))
                };
                tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
            }

            match Self::download_chunk(&job, &tmp_path).await {
                Ok(()) => {
                    utils::move_or_copy_file(&tmp_path, &job.save_path).await?;
                    return Ok(());
                }
                Err(e) => {
                    log::warn!(
                        "Worker chunk {} attempt {}/{} failed: {}",
                        job.chunk_id,
                        attempt + 1,
                        max_retries + 1,
                        e
                    );
                    let _ = tokio::fs::remove_file(&tmp_path).await;
                    last_err = Some(e);
                }
            }
        }

        Err(last_err.unwrap_or_else(|| CoreError::WorkerFailed {
            url: job.url,
            chunk_id: job.chunk_id,
            reason: format!("Download failed after {} attempts", max_retries + 1),
        }))
    }

    async fn download_chunk(job: &WorkerJob, tmp_path: &PathBuf) -> CoreResult<()> {
        let mut req = job.client.get(&job.url);
        if let Some(ref referer) = job.referer {
            if !job.url.contains("googleusercontent.com") && !job.url.contains("googlevideo.com") {
                req = req.header("Referer", referer);
            }
        }
        if let Some((start, end)) = job.range {
            req = req.header("Range", format!("bytes={}-{}", start, end));
        }

        let resp = req.send().await?;
        let status = resp.status();
        if !status.is_success() && status.as_u16() != 206 {
            return Err(CoreError::HttpStatus(status.as_u16()));
        }

        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(tmp_path)
            .await?;

        let mut resp = resp;
        let mut cipher_buf = Vec::new();

        while let Some(chunk) = resp.chunk().await? {
            job.throttler.acquire(chunk.len()).await?;
            if job.aes_key.is_some() {
                cipher_buf.extend_from_slice(&chunk);
            } else {
                file.write_all(&chunk).await?;
            }
            job.downloaded_bytes
                .fetch_add(chunk.len() as u64, Ordering::Relaxed);
        }

        if let Some(key) = &job.aes_key {
            let iv = job.aes_iv.as_ref().ok_or_else(|| {
                CoreError::Other(anyhow::anyhow!("Missing AES IV for encrypted segment"))
            })?;
            let decrypted = decrypt_aes128(&cipher_buf, key, iv)?;
            file.write_all(&decrypted).await?;
        }

        file.flush().await?;
        Ok(())
    }
}
