use std::time::Duration;
use reqwest::header::{CONTENT_TYPE, RANGE, REFERER, USER_AGENT};
use url::Url;

use crate::network::HttpClient;

/// 对媒体流地址进行通用探活校验（支持 m3u8 和 mp4 等直接媒体流）
///
/// 核心原理：
/// 1. 发送轻量 GET 请求携带 Range: bytes=0-2048（避免 HEAD 请求被部分 CDN 403 拦截，且仅拉取微量数据）
/// 2. 校验响应状态码为 200 OK 或 206 Partial Content
/// 3. 校验 Content-Type 杜绝返回 200 的防盗链/验证码 HTML 页面
/// 4. 针对 m3u8 文件，进一步校验 #EXTM3U 协议头；若为嵌套 master playlist，继续深层探测子分片列表的可达性
pub async fn probe_stream_url(
    client: &HttpClient,
    url: &str,
    referer: Option<&str>,
    ua: Option<&str>,
) -> Option<String> {
    let Ok(parsed_url) = Url::parse(url) else {
        return None;
    };

    let mut req = client
        .get(parsed_url.as_str())
        .header(RANGE, "bytes=0-2048")
        .timeout(Duration::from_secs(8));

    if let Some(r) = referer.filter(|s| !s.is_empty()) {
        if !url.contains("googleusercontent.com") && !url.contains("googlevideo.com") {
            req = req.header(REFERER, r);
        }
    }
    if let Some(u) = ua.filter(|s| !s.is_empty()) {
        req = req.header(USER_AGENT, u);
    }

    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            log::warn!("[StreamProbe] 请求失败 (url={}): {:?}", url, e);
            return None;
        }
    };

    let status = resp.status();
    if !status.is_success() && status.as_u16() != 206 {
        log::warn!("[StreamProbe] 状态码异常 (status={}, url={})", status, url);
        return None;
    }

    let headers = resp.headers();
    let content_type = headers
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_lowercase();

    // 如果服务端返回了 text/html，说明实质是报错/人机拦截页面
    if content_type.contains("text/html") {
        log::warn!("[StreamProbe] 响应为 HTML 页面而非真实视频流 (url={})", url);
        return None;
    }

    let final_url = resp.url().to_string();

    let Ok(bytes) = resp.bytes().await else {
        return None;
    };

    if bytes.is_empty() {
        log::warn!("[StreamProbe] 响应体为空 (url={})", url);
        return None;
    }

    // 1. 若为 m3u8，检查协议头与子播放列表可用性
    if url.contains(".m3u8")
        || final_url.contains(".m3u8")
        || content_type.contains("mpegurl")
        || content_type.contains("x-mpegurl")
    {
        let text = String::from_utf8_lossy(&bytes);
        if !text.contains("#EXTM3U") {
            log::warn!("[StreamProbe] m3u8 缺少 #EXTM3U 标记 (url={})", url);
            return None;
        }

        // 检查是否为嵌套 master 播放列表，如果是，验证第一个子 stream 是否存在 404
        if text.contains("#EXT-X-STREAM-INF") {
            for line in text.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }
                // trimmed 为子 playlist 地址
                let sub_url = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
                    trimmed.to_string()
                } else if let Ok(base) = Url::parse(&final_url) {
                    base.join(trimmed).map(|u| u.to_string()).unwrap_or_default()
                } else {
                    String::new()
                };

                if !sub_url.is_empty() && sub_url != final_url {
                    log::debug!("[StreamProbe] 探测 master m3u8 子分片列表: {}", sub_url);
                    let mut sub_req = client
                        .get(&sub_url)
                        .header(RANGE, "bytes=0-1024")
                        .timeout(Duration::from_secs(5));
                    if let Some(r) = referer {
                        if !sub_url.contains("googleusercontent.com") && !sub_url.contains("googlevideo.com") {
                            sub_req = sub_req.header(REFERER, r);
                        }
                    }
                    if let Some(u) = ua {
                        sub_req = sub_req.header(USER_AGENT, u);
                    }
                    if let Ok(sub_resp) = sub_req.send().await {
                        let sub_status = sub_resp.status();
                        if !sub_status.is_success() && sub_status.as_u16() != 206 {
                            log::warn!(
                                "[StreamProbe] master m3u8 子列表失效 (status={}, sub_url={})",
                                sub_status,
                                sub_url
                            );
                            return None;
                        }
                    } else {
                        log::warn!("[StreamProbe] master m3u8 子列表请求失败: {}", sub_url);
                        return None;
                    }
                }
                break; // 只需验证第一个有效子列表即可确认源站存活
            }
        }

        log::info!("[StreamProbe] m3u8 探活成功: {}", final_url);
        return Some(final_url);
    }

    // 2. 若为 MP4 或通用视频流
    log::info!(
        "[StreamProbe] 媒体流探活成功 (status={}, len={}): {} (final={})",
        status,
        bytes.len(),
        url,
        final_url
    );
    Some(final_url)
}
