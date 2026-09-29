// 处理前端的流式播放，包括 preview 和 视频

use anyhow::{anyhow, Result};
use lru::LruCache;
use parking_lot::Mutex;
use std::num::NonZeroUsize;
use std::sync::{Arc, LazyLock};
use tauri::{AppHandle, Manager};
use url::Url;

use crate::core::protocol::hls;
use crate::AppState;

#[derive(Clone)]
struct CachedMedia {
    data: Arc<Vec<u8>>,
    content_type: String,
}

// 最多缓存 60 个短预览视频（约 8MB 内存），超出自动 LRU 淘汰
static PREVIEW_CACHE: LazyLock<Mutex<LruCache<String, CachedMedia>>> =
    LazyLock::new(|| Mutex::new(LruCache::new(NonZeroUsize::new(60).unwrap())));

#[cfg(target_os = "windows")]
pub const STREAM_HOST: &str = "http://stream.localhost";

#[cfg(not(target_os = "windows"))]
pub const STREAM_HOST: &str = "stream://localhost";

fn should_omit_referer(url: &str) -> bool {
    url.contains("googleusercontent.com") || url.contains("googlevideo.com")
}

pub fn build_stream_url(
    url: &str,
    referer: Option<&str>,
    ua: Option<&str>,
    is_preview: bool,
) -> String {
    let encoded_url = url::form_urlencoded::byte_serialize(url.as_bytes()).collect::<String>();
    let mut res = format!("{}/proxy?url={}", STREAM_HOST, encoded_url);
    if let Some(r) = referer.filter(|s| !s.is_empty()) {
        let enc_ref = url::form_urlencoded::byte_serialize(r.as_bytes()).collect::<String>();
        res.push_str(&format!("&referer={}", enc_ref));
    }
    if let Some(u) = ua.filter(|s| !s.is_empty()) {
        let enc_ua = url::form_urlencoded::byte_serialize(u.as_bytes()).collect::<String>();
        res.push_str(&format!("&ua={}", enc_ua));
    }
    if is_preview {
        res.push_str("&is_preview=1");
    }
    res
}

/// 从缓存切片响应 WebKit AVFoundation 的 Range 请求
fn serve_cached_slice(
    media: &CachedMedia,
    range: Option<&str>,
) -> Result<tauri::http::Response<Vec<u8>>> {
    let total = media.data.len() as u64;
    if total == 0 {
        return Ok(tauri::http::Response::builder()
            .status(200)
            .header("Content-Type", &media.content_type)
            .header("Content-Length", "0")
            .header("Access-Control-Allow-Origin", "*")
            .body(Vec::new())?);
    }

    // 解析 Range: bytes=start-end 或 bytes=start-
    if let Some(r) = range.and_then(|s| s.trim().strip_prefix("bytes=")) {
        if let Some((start_s, end_s)) = r.split_once('-') {
            let start = start_s.trim().parse::<u64>().unwrap_or(0);
            let end = end_s
                .trim()
                .parse::<u64>()
                .unwrap_or(total.saturating_sub(1))
                .min(total.saturating_sub(1));

            if start <= end && start < total {
                let slice = media.data[start as usize..=end as usize].to_vec();
                return Ok(tauri::http::Response::builder()
                    .status(tauri::http::StatusCode::PARTIAL_CONTENT)
                    .header("Content-Type", &media.content_type)
                    .header(
                        "Content-Range",
                        format!("bytes {}-{}/{}", start, end, total),
                    )
                    .header("Content-Length", slice.len().to_string())
                    .header("Accept-Ranges", "bytes")
                    .header("Access-Control-Allow-Origin", "*")
                    .header("Access-Control-Allow-Headers", "*")
                    .header("Access-Control-Allow-Methods", "GET, HEAD, OPTIONS")
                    .header("Cache-Control", "public, max-age=604800, immutable")
                    .body(slice)?);
            }
        }
    }

    Ok(tauri::http::Response::builder()
        .status(tauri::http::StatusCode::OK)
        .header("Content-Type", &media.content_type)
        .header("Content-Length", total.to_string())
        .header("Accept-Ranges", "bytes")
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Headers", "*")
        .header("Access-Control-Allow-Methods", "GET, HEAD, OPTIONS")
        .header("Cache-Control", "public, max-age=604800, immutable")
        .body((*media.data).clone())?)
}

/// 处理前端 Webview 发起的 stream:// 媒体流请求
pub async fn handle_stream_request(
    app: AppHandle,
    request: tauri::http::Request<Vec<u8>>,
) -> Result<tauri::http::Response<Vec<u8>>> {
    if request.method() == tauri::http::Method::OPTIONS {
        return Ok(tauri::http::Response::builder()
            .status(tauri::http::StatusCode::NO_CONTENT)
            .header("Access-Control-Allow-Origin", "*")
            .header("Access-Control-Allow-Headers", "*")
            .header("Access-Control-Allow-Methods", "GET, HEAD, OPTIONS")
            .header("Access-Control-Max-Age", "86400")
            .body(Vec::new())?);
    }

    let parsed_uri = Url::parse(&request.uri().to_string().replace("stream://", "http://"))?;

    let mut target_url = String::new();
    let mut referer: Option<String> = None;
    let mut ua: Option<String> = None;
    let mut is_preview = false;

    for (k, v) in parsed_uri.query_pairs() {
        if k == "url" {
            target_url = v.into_owned();
        } else if k == "referer" {
            referer = Some(v.into_owned());
        } else if k == "ua" || k == "user_agent" {
            ua = Some(v.into_owned());
        } else if k == "is_preview" && v == "1" {
            is_preview = true;
        }
    }

    if target_url.is_empty() {
        return Ok(tauri::http::Response::builder()
            .status(tauri::http::StatusCode::BAD_REQUEST)
            .header("Access-Control-Allow-Origin", "*")
            .body(b"Missing 'url' query parameter".to_vec())?);
    }

    let is_m3u8_url = hls::is_m3u8(&target_url);
    let incoming_range = request
        .headers()
        .get("Range")
        .or_else(|| request.headers().get("range"))
        .and_then(|v| v.to_str().ok());

    // 显式标记为 preview 或 URL 包含 preview 字样，均认定为短预览
    let is_preview =
        is_preview || target_url.contains("preview") || target_url.contains("_preview");

    // 1. 命中 LRU 缓存：直接 0ms 本地内存切片返回
    if is_preview {
        let cached = PREVIEW_CACHE.lock().get(&target_url).cloned();
        if let Some(media) = cached {
            return serve_cached_slice(&media, incoming_range);
        }
    }

    let state = app.state::<AppState>();
    let mut req = state.http_client.read().get(&target_url);
    if let Some(ref r) = referer {
        if !should_omit_referer(&target_url) {
            req = req.header("Referer", r);
        }
    }
    if let Some(ref u) = ua {
        req = req.header("User-Agent", u);
    }
    // 预览短视频不需要对上游发 Range，完整拉取存入内存缓存供后续 0ms 切片；非预览视频才透传客户端 Range
    if !is_preview {
        if let Some(r) = incoming_range {
            req = req.header("Range", r);
        }
    }

    let upstream_resp = req.send().await?;
    let status_code = upstream_resp.status().as_u16();
    let upstream_headers = upstream_resp.headers().clone();

    // 2. 处理 M3U8 播放列表重写
    if is_m3u8_url {
        let text = upstream_resp.text().await.unwrap_or_default();
        let rewritten =
            rewrite_m3u8_for_streaming(&target_url, referer.as_deref(), ua.as_deref(), &text)?;
        return Ok(tauri::http::Response::builder()
            .status(200)
            .header("Content-Type", "application/vnd.apple.mpegurl")
            .header("Access-Control-Allow-Origin", "*")
            .header("Access-Control-Allow-Headers", "*")
            .header("Access-Control-Allow-Methods", "GET, HEAD, OPTIONS")
            .header("Cache-Control", "no-cache")
            .body(rewritten.into_bytes())?);
    }

    let raw_bytes = upstream_resp.bytes().await.unwrap_or_default().to_vec();

    // 3. 如果是 preview 视频，写入 LRU 缓存（上限 60 个）
    if is_preview && status_code < 400 && raw_bytes.len() <= 15 * 1024 * 1024 {
        let content_type = if target_url.contains(".mp4") {
            "video/mp4".to_string()
        } else {
            upstream_headers
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("video/mp4")
                .to_string()
        };
        let media = CachedMedia {
            data: Arc::new(raw_bytes),
            content_type,
        };
        PREVIEW_CACHE.lock().put(target_url, media.clone());
        return serve_cached_slice(&media, incoming_range);
    }

    // 4. 构建流式响应：透传类型与真实长度
    let content_type = if target_url.contains(".mp4") {
        "video/mp4".to_string()
    } else if target_url.contains(".ts") {
        "video/mp2t".to_string()
    } else {
        upstream_headers
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_string()
    };

    #[cfg(debug_assertions)]
    if raw_bytes.len() < 512 && raw_bytes.len() != 16 && status_code == 200 {
        log::error!(
            "[StreamProtocol] 上游返回异常短响应 (len={}): url={}, body={}",
            raw_bytes.len(),
            target_url,
            String::from_utf8_lossy(&raw_bytes)
        );
    }

    let mut resp_builder = tauri::http::Response::builder()
        .status(status_code)
        .header("Content-Type", &content_type)
        .header("Content-Length", raw_bytes.len().to_string())
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Headers", "*")
        .header("Access-Control-Allow-Methods", "GET, HEAD, OPTIONS")
        .header("Cache-Control", "public, max-age=604800, immutable");

    if let Some(cr) = upstream_headers.get("content-range") {
        resp_builder = resp_builder.header("Content-Range", cr);
    }
    if let Some(ar) = upstream_headers.get("accept-ranges") {
        resp_builder = resp_builder.header("Accept-Ranges", ar);
    }

    Ok(resp_builder.body(raw_bytes)?)
}

/// 将 M3U8 文本中的分片 URI、密钥 URI、子流 URI 重写为 stream:// 代理协议 URL
pub fn rewrite_m3u8_for_streaming(
    base_url: &str,
    referer: Option<&str>,
    ua: Option<&str>,
    content: &str,
) -> Result<String> {
    let base =
        Url::parse(base_url).map_err(|e| anyhow!("Invalid base url '{}': {}", base_url, e))?;

    let mut extra_params = String::new();
    if let Some(r) = referer {
        let enc = url::form_urlencoded::byte_serialize(r.as_bytes()).collect::<String>();
        extra_params.push_str(&format!("&referer={}", enc));
    }
    if let Some(u) = ua {
        let enc = url::form_urlencoded::byte_serialize(u.as_bytes()).collect::<String>();
        extra_params.push_str(&format!("&ua={}", enc));
    }

    let to_stream_proxy_url = |raw_uri: &str| -> String {
        let resolved = base
            .join(raw_uri)
            .map(|u| u.to_string())
            .unwrap_or_else(|_| raw_uri.to_string());
        let encoded = url::form_urlencoded::byte_serialize(resolved.as_bytes()).collect::<String>();
        format!("{}/proxy?url={}{}", STREAM_HOST, encoded, extra_params)
    };

    hls::rewrite_playlist_uris(content.as_bytes(), to_stream_proxy_url).map_err(|e| anyhow!(e))
}
