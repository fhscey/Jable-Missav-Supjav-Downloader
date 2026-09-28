use crate::core::error::CoreResult;
use crate::core::network::HttpClient;

/// HTTP 直接下载流的探测元信息
#[derive(Debug, Clone)]
pub struct HttpStreamInfo {
    pub content_length: Option<u64>,
    pub accept_ranges: bool,
    pub content_type: Option<String>,
}

/// 探测常规 HTTP 二进制文件
pub async fn probe_http_stream(
    client: &HttpClient,
    url: &str,
    referer: Option<&str>,
) -> CoreResult<HttpStreamInfo> {
    let mut req = client.head(url);
    if let Some(r) = referer {
        req = req.header("Referer", r);
    }
    let resp = req.send().await?;

    let content_length = resp
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());

    let accept_ranges = resp
        .headers()
        .get(reqwest::header::ACCEPT_RANGES)
        .and_then(|v| v.to_str().ok())
        .map_or(false, |v| v.eq_ignore_ascii_case("bytes"));

    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_string());

    Ok(HttpStreamInfo {
        content_length,
        accept_ranges,
        content_type,
    })
}
