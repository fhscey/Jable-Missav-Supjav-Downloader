// 处理前端的流式播放，包括 preview 和 视频

use anyhow::{anyhow, Result};
use lru::LruCache;
use parking_lot::Mutex;
use std::num::NonZeroUsize;
use std::sync::{Arc, LazyLock};
use tauri::{AppHandle, Manager};
use url::Url;

use crate::core::protocol::hls;
use crate::utils::strip_image_disguise;
use crate::AppState;

#[derive(Clone)]
struct CachedMedia {
    data: Arc<Vec<u8>>,
    content_type: String,
}

// 最多缓存 60 个短预览视频（约 8MB 内存），超出自动 LRU 淘汰
static PREVIEW_CACHE: LazyLock<Mutex<LruCache<String, CachedMedia>>> = LazyLock::new(|| {
    Mutex::new(LruCache::new(NonZeroUsize::new(60).unwrap()))
});

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
    let mut res = format!("stream://localhost/proxy?url={}", encoded_url);
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
                    .header("Content-Range", format!("bytes {}-{}/{}", start, end, total))
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
    let parsed_uri = Url::parse(&request.uri().to_string().replace("stream://", "http://"))?;

    let mut target_url = String::new();
    let mut referer: Option<String> = None;
    let mut ua: Option<String> = None;
    let mut is_preview = false;
    let mut is_seg = false;

    for (k, v) in parsed_uri.query_pairs() {
        if k == "url" {
            target_url = v.into_owned();
        } else if k == "referer" {
            referer = Some(v.into_owned());
        } else if k == "ua" || k == "user_agent" {
            ua = Some(v.into_owned());
        } else if k == "is_preview" && v == "1" {
            is_preview = true;
        } else if k == "is_seg" && v == "1" {
            is_seg = true;
        }
    }

    if target_url.is_empty() {
        return Ok(tauri::http::Response::builder()
            .status(tauri::http::StatusCode::BAD_REQUEST)
            .header("Access-Control-Allow-Origin", "*")
            .body(b"Missing 'url' query parameter".to_vec())?);
    }

    let is_m3u8_url = hls::is_m3u8(&target_url);
    let is_seg = is_seg || target_url.contains(".ts") || target_url.contains(".key");
    let incoming_range = request
        .headers()
        .get("Range")
        .or_else(|| request.headers().get("range"))
        .and_then(|v| v.to_str().ok());

    // 显式标记为 preview 或 URL 包含 preview 字样，均认定为短预览
    let is_preview = is_preview || target_url.contains("preview") || target_url.contains("_preview");

    // 1. 命中 LRU 缓存：直接 0ms 本地内存切片返回
    if is_preview {
        let cached = PREVIEW_CACHE.lock().get(&target_url).cloned();
        if let Some(media) = cached {
            log::debug!("[StreamProtocol] 命中内存切片缓存: target_url={}, range={:?}", target_url, incoming_range);
            return serve_cached_slice(&media, incoming_range);
        }
    }

    log::debug!(
        "[StreamProtocol] 发起上游请求: target_url={}, is_preview={}, is_m3u8={}, range={:?}",
        target_url,
        is_preview,
        is_m3u8_url,
        incoming_range
    );

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

    // 2. 如果是正片大视频（且非 m3u8、非切片），限制 Range 为 1.5MB 滑动窗口，极速起播首帧且零内存常驻
    if !is_m3u8_url && !is_preview && !is_seg {
        let chunk_size: u64 = 1_572_864;
        let effective_range = if let Some(r) = incoming_range {
            let r_trimmed = r.trim();
            if let Some(spec) = r_trimmed.strip_prefix("bytes=") {
                if let Some((start_s, end_s)) = spec.split_once('-') {
                    if let Ok(start) = start_s.trim().parse::<u64>() {
                        if end_s.trim().is_empty() {
                            Some(format!("bytes={}-{}", start, start + chunk_size - 1))
                        } else {
                            Some(r_trimmed.to_string())
                        }
                    } else {
                        Some(r_trimmed.to_string())
                    }
                } else {
                    Some(r_trimmed.to_string())
                }
            } else {
                Some(r_trimmed.to_string())
            }
        } else {
            Some(format!("bytes=0-{}", chunk_size - 1))
        };
        if let Some(r_val) = effective_range {
            req = req.header("Range", r_val);
        }
    }

    let upstream_resp = req.send().await?;
    let status_code = upstream_resp.status().as_u16();
    let upstream_headers = upstream_resp.headers().clone();
    log::debug!("[StreamProtocol] 上游返回响应: target_url={}, status={}", target_url, status_code);

    // 3. 处理 M3U8 播放列表重写
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

    // 4. 如果是 preview 视频，写入 LRU 缓存（上限 60 个），后续 Range 探测瞬间从内存命中
    if is_preview && status_code < 400 && raw_bytes.len() <= 15 * 1024 * 1024 {
        let content_type = if target_url.contains(".mp4") {
            "video/mp4"
        } else {
            upstream_headers
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("video/mp4")
        };
        let media = CachedMedia {
            data: Arc::new(raw_bytes),
            content_type: content_type.to_string(),
        };
        PREVIEW_CACHE.lock().put(target_url, media.clone());
        return serve_cached_slice(&media, incoming_range);
    }

    // 5. 检查并脱去可能存在的图片伪装（PNG / JPEG 伪装头，例如 Google CDN 上的 TS 分片）
    let raw_len = raw_bytes.len();
    let stripped = strip_image_disguise(&raw_bytes);
    let is_ts_stream = !stripped.is_empty() && stripped[0] == 0x47;

    let (final_bytes, content_type) = if is_ts_stream {
        (stripped.to_vec(), "video/mp2t".to_string())
    } else if target_url.contains(".mp4") {
        (raw_bytes, "video/mp4".to_string())
    } else {
        let ct = upstream_headers
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("video/mp4")
            .to_string();
        (raw_bytes, ct)
    };

    // 6. 流式媒体响应构建
    let mut resp_builder = tauri::http::Response::builder()
        .header("Content-Type", &content_type)
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Headers", "*")
        .header("Access-Control-Allow-Methods", "GET, HEAD, OPTIONS")
        .header("Cache-Control", "public, max-age=604800, immutable");

    if is_ts_stream && incoming_range.is_none() {
        // TS 分片完整响应时强制修正为 200 与脱壳后的真实 Content-Length
        resp_builder = resp_builder
            .status(200)
            .header("Content-Length", final_bytes.len().to_string());
    } else {
        resp_builder = resp_builder.status(status_code);
        if let Some(cr) = upstream_headers.get("content-range") {
            resp_builder = resp_builder.header("Content-Range", cr);
        }
        if let Some(ar) = upstream_headers.get("accept-ranges") {
            resp_builder = resp_builder.header("Accept-Ranges", ar);
        }
        if let Some(cl) = upstream_headers.get("content-length") {
            if final_bytes.len() != raw_len {
                resp_builder = resp_builder.header("Content-Length", final_bytes.len().to_string());
            } else {
                resp_builder = resp_builder.header("Content-Length", cl);
            }
        }
    }

    Ok(resp_builder.body(final_bytes)?)
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
        let is_sub_m3u8 = hls::is_m3u8(&resolved);
        let seg_param = if !is_sub_m3u8 { "&is_seg=1" } else { "" };
        format!("stream://localhost/proxy?url={}{}{}", encoded, extra_params, seg_param)
    };

    hls::rewrite_playlist_uris(content.as_bytes(), to_stream_proxy_url).map_err(|e| anyhow!(e))
}
