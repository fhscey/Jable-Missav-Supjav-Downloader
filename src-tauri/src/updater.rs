use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::network::{HttpClient, DEFAULT_UA};

/// 检查更新结果，返回给前端展示
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
    pub changelog: String,
    pub release_url: String,
    pub published_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    #[serde(default)]
    body: String,
    html_url: String,
    published_at: Option<String>,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    draft: bool,
}

impl GithubRelease {
    #[inline]
    fn is_stable(&self) -> bool {
        !self.prerelease && !self.draft
    }
}

const DEFAULT_REPO: &str = "shurgogo/Jable-Missav-Supjav-Downloader";
const DEFAULT_API_BASE: &str =
    "https://api.github.com/repos/shurgogo/Jable-Missav-Supjav-Downloader";

fn api_base() -> String {
    std::env::var("AVDL_RELEASE_API_URL").unwrap_or_else(|_| DEFAULT_API_BASE.to_string())
}

fn repo_slug() -> String {
    let base = api_base();
    if let Some(pos) = base.find("repos/") {
        base[pos + 6..].trim_matches('/').to_string()
    } else {
        DEFAULT_REPO.to_string()
    }
}

/// 语义化版本比对（支持 "1.2.3", "v1.2.3", "0.1.5-beta.1" 等）
/// 当 a 严格大于 b 时返回 true
pub fn is_newer(a: &str, b: &str) -> bool {
    fn parse(v: &str) -> (u64, u64, u64) {
        let core = v
            .trim()
            .trim_start_matches('v')
            .split(['-', '+'])
            .next()
            .unwrap_or("0");
        let mut parts = core.split('.');
        let major = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        let minor = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        let patch = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        (major, minor, patch)
    }
    parse(a) > parse(b)
}

async fn fetch_release<T: for<'de> Deserialize<'de>>(
    client: &HttpClient,
    url: &str,
) -> Result<T, String> {
    log::debug!("[Updater] 开始向 GitHub API 发起请求: url={}", url);
    let resp = client
        .get(url)
        .header("user-agent", DEFAULT_UA)
        .header("accept", "application/vnd.github+json")
        .header("x-github-api-version", "2022-11-28")
        .send()
        .await
        .map_err(|e| {
            log::error!("[Updater] 网络请求发生错误: url={}, error={:?}", url, e);
            format!("网络请求失败: {}", e)
        })?;

    let status = resp.status();
    log::debug!("[Updater] 收到响应状态码: url={}, status={}", url, status);

    // 打印关键 GitHub 响应头（如限流状态信息）
    if let Some(remaining) = resp.headers().get("x-ratelimit-remaining") {
        let limit = resp
            .headers()
            .get("x-ratelimit-limit")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("unknown");
        let reset = resp
            .headers()
            .get("x-ratelimit-reset")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("unknown");
        log::debug!(
            "[Updater] GitHub RateLimit: remaining={:?}, limit={}, reset={}",
            remaining.to_str().unwrap_or(""),
            limit,
            reset
        );
    }

    let body = resp.text().await.map_err(|e| {
        log::error!("[Updater] 读取响应 Body 失败: url={}, error={:?}", url, e);
        format!("读取响应失败: {}", e)
    })?;

    if !status.is_success() {
        log::warn!(
            "[Updater] GitHub API 响应非成功状态: url={}, status={}, body={}",
            url,
            status,
            body
        );
        return Err(format!("GitHub API 返回 HTTP {}: {}", status, body.trim()));
    }

    log::debug!(
        "[Updater] 请求成功并收到响应体: url={}, body_len={}",
        url,
        body.len()
    );

    serde_json::from_str::<T>(&body).map_err(|e| {
        log::error!(
            "[Updater] 反序列化 JSON 响应失败: url={}, error={:?}, body={}",
            url,
            e,
            body
        );
        format!("解析响应失败: {}", e)
    })
}

/// 解析 GitHub Releases Atom 源，提取最新的 release 信息（免 GitHub API 60次/小时限流）
pub fn parse_atom_feed(xml: &str, repo: &str) -> Result<UpdateInfo, String> {
    let entry_start = xml
        .find("<entry>")
        .ok_or_else(|| "Atom 源中未找到任何 release entry".to_string())?;
    let entry_end = xml[entry_start..]
        .find("</entry>")
        .map(|idx| entry_start + idx)
        .unwrap_or(xml.len());
    let entry = &xml[entry_start..entry_end];

    let tag_marker = "/releases/tag/";
    let tag = if let Some(pos) = entry.find(tag_marker) {
        let after = &entry[pos + tag_marker.len()..];
        let end = after
            .find(['"', '\'', ' ', '<', '>'])
            .unwrap_or(after.len());
        after[..end].trim().to_string()
    } else if let Some(id_start) = entry.find("<id>") {
        let after = &entry[id_start + 4..];
        let id_val = after.split("</id>").next().unwrap_or("").trim();
        id_val.rsplit('/').next().unwrap_or("").to_string()
    } else {
        return Err("无法从 Atom entry 中解析 release tag".to_string());
    };

    if tag.is_empty() {
        return Err("解析出的 tag 为空".to_string());
    }

    let published_at = if let Some(up_start) = entry.find("<updated>") {
        let after = &entry[up_start + 9..];
        after
            .split("</updated>")
            .next()
            .map(|s| s.trim().to_string())
    } else {
        None
    };

    let release_url = format!("https://github.com/{}/releases/tag/{}", repo, tag);
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let update_available = is_newer(&tag, &current_version);

    Ok(UpdateInfo {
        current_version,
        latest_version: tag,
        update_available,
        changelog: String::new(),
        release_url,
        published_at,
    })
}

/// 通过 GitHub Releases Atom 源兜底检查更新（无需 API Token，不受 60次/小时 配额限制）
async fn check_via_atom_feed(client: &HttpClient, repo: &str) -> Result<UpdateInfo, String> {
    let atom_url = format!("https://github.com/{}/releases.atom", repo);
    log::debug!(
        "[Updater] 尝试通过 GitHub Releases Atom 源检查更新: {}",
        atom_url
    );

    let resp = client
        .get(&atom_url)
        .header("user-agent", DEFAULT_UA)
        .send()
        .await
        .map_err(|e| format!("请求 Atom 源失败: {}", e))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(format!("GitHub Atom 源返回 HTTP {}", status));
    }

    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取 Atom 响应失败: {}", e))?;

    parse_atom_feed(&text, repo)
}

/// 通过 GitHub Web Releases 页面重定向兜底检查（/releases/latest 自动 302 重定向至最新正式版）
async fn check_via_web_redirect(client: &HttpClient, repo: &str) -> Result<UpdateInfo, String> {
    let latest_web_url = format!("https://github.com/{}/releases/latest", repo);
    log::debug!(
        "[Updater] 尝试通过 GitHub Web 重定向检测最新版本: {}",
        latest_web_url
    );

    let resp = client
        .get(&latest_web_url)
        .header("user-agent", DEFAULT_UA)
        .send()
        .await
        .map_err(|e| format!("请求 GitHub Web 重定向失败: {}", e))?;

    let final_url = resp.url().as_str();
    log::debug!("[Updater] Web 重定向最终 URL: {}", final_url);

    let tag_marker = "/releases/tag/";
    if let Some(pos) = final_url.find(tag_marker) {
        let tag = final_url[pos + tag_marker.len()..]
            .trim_matches('/')
            .to_string();
        if !tag.is_empty() {
            let current_version = env!("CARGO_PKG_VERSION").to_string();
            let update_available = is_newer(&tag, &current_version);
            return Ok(UpdateInfo {
                current_version,
                latest_version: tag.clone(),
                update_available,
                changelog: String::new(),
                release_url: format!("https://github.com/{}/releases/tag/{}", repo, tag),
                published_at: None,
            });
        }
    }

    Err(format!("无法从重定向 URL ({}) 中解析版本号", final_url))
}

/// 负责检测应用版本与更新的服务组件
pub struct Updater {
    http_client: Arc<RwLock<HttpClient>>,
}

impl Updater {
    pub fn new(http_client: Arc<RwLock<HttpClient>>) -> Self {
        Self { http_client }
    }

    /// 检查应用是否有可用正式版本
    /// 策略：优先走 GitHub REST API，如果遇到 403 限流或网络异常，自动无缝降级至 Atom 源 / Web 302 重定向
    pub async fn check(&self) -> Result<UpdateInfo, String> {
        let client = self.http_client.read().clone();
        let base = api_base();
        let latest_url = format!("{}/releases/latest", base);
        let list_url = format!("{}/releases?per_page=10", base);
        let repo = repo_slug();

        log::info!("[Updater] 正在向 GitHub 检查更新: repo={}", repo);

        // 1. 尝试通过 GitHub REST API 检查
        let api_result: Result<UpdateInfo, String> = async {
            log::debug!("[Updater] [API通道] 准备请求 latest release 地址: {}", latest_url);
            let target = match fetch_release::<GithubRelease>(&client, &latest_url).await {
                Ok(release) if release.is_stable() => {
                    log::debug!("[Updater] [API通道] 成功获取 latest 正式发布版本: tag_name={}", release.tag_name);
                    release
                }
                Ok(release) => {
                    log::debug!(
                        "[Updater] [API通道] latest 返回版本不是正式版: tag_name={}, prerelease={}, draft={}, 转向 releases 列表: {}",
                        release.tag_name,
                        release.prerelease,
                        release.draft,
                        list_url
                    );
                    let list = fetch_release::<Vec<GithubRelease>>(&client, &list_url).await?;
                    list.into_iter()
                        .find(|r| r.is_stable())
                        .ok_or_else(|| "没有可用的正式版本".to_string())?
                }
                Err(e) => {
                    log::warn!(
                        "[Updater] [API通道] 请求 latest_url 失败: {}, 尝试拉取 releases 列表: {}",
                        e,
                        list_url
                    );
                    let list = fetch_release::<Vec<GithubRelease>>(&client, &list_url).await?;
                    list.into_iter()
                        .find(|r| r.is_stable())
                        .ok_or_else(|| "没有可用的正式版本".to_string())?
                }
            };

            let current_version = env!("CARGO_PKG_VERSION").to_string();
            let update_available = is_newer(&target.tag_name, &current_version);

            Ok(UpdateInfo {
                current_version,
                latest_version: target.tag_name,
                update_available,
                changelog: target.body,
                release_url: target.html_url,
                published_at: target.published_at,
            })
        }
        .await;

        match api_result {
            Ok(info) => {
                log::debug!(
                    "[Updater] [API] 版本检查成功: 当前版本={}, 最新版本={}, 有更新={}",
                    info.current_version,
                    info.latest_version,
                    info.update_available
                );
                Ok(info)
            }
            Err(api_err) => {
                log::warn!(
                    "[Updater] GitHub REST API 访问受限或失败 ({})，自动切换至 GitHub Atom / Web 兜底通道...",
                    api_err
                );

                // 2. 兜底通道 A: Releases Atom 源（无 API 60次/小时限制，支持代理与 VPN 环境）
                match check_via_atom_feed(&client, &repo).await {
                    Ok(info) => {
                        log::debug!(
                            "[Updater] [Atom] 检查成功: 当前版本={}, 最新版本={}, 有更新={}",
                            info.current_version,
                            info.latest_version,
                            info.update_available
                        );
                        Ok(info)
                    }
                    Err(atom_err) => {
                        log::warn!(
                            "[Updater] Atom 兜底通道失败 ({})，尝试 Web 302 重定向兜底...",
                            atom_err
                        );

                        // 3. 兜底通道 B: Web 重定向（无任何 API 限制，仅根据 302 URL 提取 tag）
                        check_via_web_redirect(&client, &repo)
                            .await
                            .map_err(|web_err| {
                                log::error!(
                                    "[Updater] 所有更新检查通道均失败: api={}, atom={}, web={}",
                                    api_err,
                                    atom_err,
                                    web_err
                                );
                                format!("检查更新失败: {}", api_err)
                            })
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_newer() {
        assert!(is_newer("v1.0.1", "1.0.0"));
        assert!(is_newer("1.2.0", "1.1.9"));
        assert!(is_newer("2.0.0", "1.9.9"));
        assert!(is_newer("0.2.0-beta.1", "0.1.9"));
        assert!(!is_newer("1.0.0", "1.0.0"));
        assert!(!is_newer("v1.0.0", "1.0.0"));
        assert!(!is_newer("0.1.0", "0.1.1"));
        assert!(!is_newer("0.1.0", "1.0.0"));
    }

    #[test]
    fn test_parse_atom_feed() {
        let sample = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <entry>
    <id>tag:github.com,2008:Repository/1309780715/v0.3.0</id>
    <updated>2026-08-17T05:09:07Z</updated>
    <link rel="alternate" type="text/html" href="https://github.com/shurgogo/Jable-Missav-Supjav-Downloader/releases/tag/v0.3.0"/>
    <title>AVDL v0.3.0</title>
  </entry>
</feed>"#;
        let info = parse_atom_feed(sample, "shurgogo/Jable-Missav-Supjav-Downloader").unwrap();
        assert_eq!(info.latest_version, "v0.3.0");
        assert_eq!(
            info.release_url,
            "https://github.com/shurgogo/Jable-Missav-Supjav-Downloader/releases/tag/v0.3.0"
        );
        assert_eq!(info.published_at.as_deref(), Some("2026-08-17T05:09:07Z"));
        assert!(info.update_available);
    }
}
