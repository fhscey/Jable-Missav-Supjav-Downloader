use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::Duration;

use async_trait::async_trait;
use regex::Regex;

use super::SiteProvider;
use crate::config::Language;
use crate::extractor::error::ExtractorError;
use crate::extractor::model::{
    FetchParams, MediaDetail, NavItem, ProviderSite, SiteManifest, SortOption, SortRule, TagGroup,
    VideoPage,
};
use crate::extractor::probe_stream_url;
use crate::extractor::VideoInfo;
use crate::network::{HttpClient, WebviewEngine};
use crate::utils;
use dom_query::{Document, Selection};

const EXTRACT_READY_CONDITION_SUPJAV: &str = "document.querySelector('div.btns a.btn-server') || document.querySelector('div#dz_video') || (document.querySelector('div.archive-title h1') && document.querySelector('div.post-meta'))";
const LIST_READY_CONDITION_SUPJAV: &str = "document.querySelectorAll('div.post img').length >= 10 || (document.readyState === 'complete' && (document.querySelectorAll('div.post img').length > 0 || document.querySelector('div.posts') || document.querySelector('div.content')))";

static RE_URL_PLAY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"urlPlay[\s=:\'"]+(https?://[^\s'"\\]+\.m3u8[^\s'"\\]*)"#).unwrap()
});
static RE_SOURCE_M3U8: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?:file|source)[\s=:\'"]+(https?://[^\s'"\\]+\.m3u8[^\s'"\\]*)"#).unwrap()
});
static RE_FALLBACK_M3U8: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(https?://[^\s'"\\]+\.m3u8[^\s'"\\]*)"#).unwrap());
static RE_TAPECONTENT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(https?://[a-zA-Z0-9.\-_]+\.tapecontent\.net/[^\s'"\\]+)"#).unwrap()
});
static RE_MP4_STREAM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(https?://[^\s'"\\]+\.mp4(?:\?[^\s'"\\]*)?)"#).unwrap());
static RE_SUBSTRING_CALL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^\s*\.(?:substring|substr|slice)\(\s*(\d+)\s*(?:,\s*(\d+)\s*)?\)"#).unwrap()
});

pub struct SupjavProvider {
    domains: Vec<String>,
    manifest: SiteManifest,
}

impl SupjavProvider {
    pub fn default_domains() -> Vec<String> {
        vec!["https://supjav.com".to_string()]
    }

    pub fn new(domains: Vec<String>) -> Self {
        let domains = if domains.is_empty() {
            Self::default_domains()
        } else {
            domains
        };
        let manifest = Self::build_manifest(&domains[0], &domains);
        Self { domains, manifest }
    }

    fn build_manifest(active_domain: &str, domains: &[String]) -> SiteManifest {
        let domain_clean = active_domain.trim_end_matches('/');
        SiteManifest {
            site: ProviderSite::Supjav,
            name: "SupJAV".to_string(),
            primary_domain: domain_clean.to_string(),
            available_domains: domains.to_vec(),
            default_url: format!("{}/", domain_clean),
            quick_links: get_supjav_quick_links(domain_clean),
            categories: get_supjav_categories(domain_clean),
            tags: get_supjav_tags(domain_clean),
            supported_languages: HashMap::from([
                (Language::EN, "en".to_string()),
                (Language::JA, "ja".to_string()),
                (Language::ZHTW, "zh".to_string()),
                (Language::ZHCN, "zh".to_string()),
            ]),
            sort_rules: get_supjav_sort_rules(),
        }
    }

    fn parse_video_card(node: &Selection) -> Option<VideoInfo> {
        let title_link = node.select("h3 a");
        let title = Some(title_link.text().trim().to_string()).filter(|s| !s.is_empty())?;
        let detail_page_url = title_link
            .attr("href")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())?;

        let img = node.select("img.thumb");
        // 缩略图
        // 优先取 data-original，src 有可能是占位符
        let thumbnail_url = img
            .attr("data-original")
            .or_else(|| img.attr("src"))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())?;
        // 高清图
        let cover_url = thumbnail_url
            .strip_suffix("!320x216.jpg")
            .map(|s| s.to_string())
            .unwrap_or(thumbnail_url);

        Some(VideoInfo {
            id: detail_page_url.clone(),
            title,
            cover_url,
            detail_page_url,
            duration: None,
            preview_url: None,
            referer: None,
            ua: None,
        })
    }

    // https://supjav.com/zh/category/uncensored-jav/page/4?sort=date
    fn build_url(
        &self,
        raw_url: &str,
        params: Option<&FetchParams>,
    ) -> Result<String, ExtractorError> {
        let Some(params) = params else {
            return Ok(raw_url.to_string());
        };
        let mut url = url::Url::parse(raw_url)
            .map_err(|_| ExtractorError::UnsupportedUrl(raw_url.to_string()))?;

        let path = url.path().to_string();

        let mut query_map: std::collections::BTreeMap<String, String> = url
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.trim_end_matches('/').to_string()))
            .collect();

        if let Some(sort_by) = &params.sort_by {
            if !sort_by.is_empty() {
                query_map.insert("sort".to_string(), sort_by.clone());
            }
        }

        url.set_query(None);
        if !query_map.is_empty() {
            let mut qp = url.query_pairs_mut();
            for (k, v) in &query_map {
                qp.append_pair(k, v);
            }
        }

        // 2. 处理 path 中的 lang 和 page
        let new_path = self.rebuild_path(&path, params)?;
        url.set_path(&new_path);

        Ok(url.to_string())
    }

    fn rebuild_path(&self, path: &str, params: &FetchParams) -> Result<String, ExtractorError> {
        let mut segments: Vec<String> = path
            .split('/')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();

        let lang_map = &self.manifest.supported_languages;

        let is_lang = |s: &str| lang_map.values().any(|v| v == s);

        // 1. 删除旧的语言段
        if !segments.is_empty() && is_lang(&segments[0]) {
            segments.remove(0);
        }

        // 2. 删除旧的 /page/{n}
        if let Some(pos) = segments.iter().position(|s| s == "page") {
            // 删除 "page"
            segments.remove(pos);
            // 删除后面的数字（如果存在）
            if pos < segments.len() {
                segments.remove(pos);
            }
        }

        // 头部插入 lang
        if let Some(lang) = params.lang {
            let code = lang_map
                .get(&lang)
                .ok_or_else(|| ExtractorError::UnsupportedLanguage(lang))?
                .clone();

            segments.insert(0, code);
        }

        // 尾部插入新 page
        if let Some(page) = params.page {
            segments.push("page".into());
            segments.push(page.to_string());
        }

        Ok(format!("/{}", segments.join("/")))
    }
}

#[async_trait]
impl SiteProvider for SupjavProvider {
    fn site(&self) -> ProviderSite {
        ProviderSite::Supjav
    }

    fn domains(&self) -> &[String] {
        &self.domains
    }

    fn manifest(&self) -> &SiteManifest {
        &self.manifest
    }

    fn can_handle(&self, url: &str) -> bool {
        if url.contains("supjav") {
            return true;
        }
        self.domains.iter().any(|d| {
            if let Ok(u) = url::Url::parse(d) {
                u.host_str().map_or(false, |h| url.contains(h))
            } else {
                false
            }
        })
    }

    fn search_url(&self, keyword: &str, params: Option<&FetchParams>) -> Option<String> {
        let encoded_keyword = utils::url_encode(keyword);
        let primary_domain = self.primary_domain();
        // 这里偷懒了，由于 ?s= 只服务于 supjav，所以没有做成 params
        self.build_url(
            format!("{}/?s={}", primary_domain, encoded_keyword).as_str(),
            params,
        )
        .ok()
    }

    async fn list(
        &self,
        webview: &WebviewEngine,
        url: &str,
        params: Option<&FetchParams>,
    ) -> Result<VideoPage, ExtractorError> {
        let current_page = params.and_then(|p| p.page).unwrap_or(1);

        let url_with_params = self.build_url(url, params)?;
        let page = webview
            .fetch_page(&url_with_params, LIST_READY_CONDITION_SUPJAV)
            .await
            .map_err(|e| ExtractorError::Other(e.to_string()))?;
        let doc = Document::from(page.html.as_str());

        // 计算值和提取值，互补
        let total_pages = {
            let calculated = doc
                .select("div.archive-title h1")
                .first()
                .text()
                .split(|c: char| !c.is_ascii_digit())
                .find_map(|s| s.parse::<usize>().ok())
                .map(|s| s.div_ceil(24))
                .unwrap_or(1);
            let extracted = doc
                .select("div.pagination a")
                .iter()
                .filter_map(|node| {
                    let text = node.text().trim().to_string();
                    if text.is_empty() {
                        return None;
                    }
                    text.chars()
                        .all(|c| c.is_ascii_digit())
                        .then_some(text.parse::<usize>().ok())?
                })
                .max()
                .unwrap_or(1);
            calculated.max(extracted)
        };

        log::debug!("[Supjav] Total pages {total_pages} of {url_with_params}");
        log::debug!("[Supjav] The redirected/final url is {}", page.final_url);

        let mut videos: Vec<VideoInfo> = doc
            .select("div.post")
            .iter()
            .filter_map(|node| Self::parse_video_card(&node))
            .collect();

        for video in &mut videos {
            video.referer = Some(page.final_url.clone());
            video.ua = page.user_agent.clone();
        }

        Ok(VideoPage {
            items: videos,
            page: current_page,
            total_pages,
        })
    }

    async fn extract(
        &self,
        webview: &WebviewEngine,
        client: &HttpClient,
        detail_page_url: &str,
        params: Option<&FetchParams>,
    ) -> Result<MediaDetail, ExtractorError> {
        let url_with_params = self.build_url(detail_page_url, params)?;

        let html = webview
            .fetch_page(&url_with_params, EXTRACT_READY_CONDITION_SUPJAV)
            .await
            .map_err(|e| ExtractorError::Other(e.to_string()))?;

        let (title, cover_url, categories, actresses, tags, data_links) = {
            let doc = Document::from(html.html.as_str());

            let metadata = doc.select("div.content div.post-meta");
            let title = Some(metadata.select("h2").text().trim().to_string())
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    ExtractorError::HtmlParse("title".to_string(), url_with_params.to_string())
                })?;

            let cover_url = metadata
                .select("img")
                .attr("src")
                .map(|v| v.to_string())
                .ok_or_else(|| {
                    ExtractorError::HtmlParse("cover_url".to_string(), url_with_params.to_string())
                })?;

            let categories: Vec<NavItem> = metadata
                .select("p.cat a")
                .iter()
                .filter_map(|a| {
                    let name = a.text().trim().to_string();
                    let url = a.attr("href")?.trim().to_string();
                    (!name.is_empty() && !url.is_empty()).then_some(NavItem { name, url })
                })
                .collect();

            let actresses: Vec<NavItem> = metadata
                .select("div.cats p a")
                .iter()
                .filter_map(|a| {
                    let name = a.text().trim().to_string();
                    let url = a.attr("href")?.trim().to_string();
                    (!name.is_empty() && !url.is_empty() && url.contains("category/cast"))
                        .then_some(NavItem { name, url })
                })
                .collect();

            let tags: Vec<NavItem> = metadata
                .select("div.tags a")
                .iter()
                .filter_map(|a| {
                    let name = a.text().trim().to_string();
                    let url = a.attr("href")?.trim().to_string();
                    (!name.is_empty() && !url.is_empty()).then_some(NavItem { name, url })
                })
                .collect();

            let data_links: Vec<String> = doc
                .select("div.btnst a")
                .iter()
                .filter_map(|a| {
                    let link = a.attr("data-link")?.trim().to_string();
                    (!link.is_empty()).then_some(link)
                })
                .collect();

            Ok::<_, ExtractorError>((title, cover_url, categories, actresses, tags, data_links))
        }?;

        // supjav 的 referer，不是当前页面，而是根据 data_link 来定
        let (referer, stream_url) =
            resolve_server_stream(client, &data_links, html.user_agent.as_deref())
                .await
                .map_err(|e| {
                    log::error!(
                        "[SupjavProvider] 提取视频播放流失败 (url={}): {:?}",
                        url_with_params,
                        e
                    );
                    ExtractorError::HtmlParse("stream_url".to_string(), url_with_params.to_string())
                })?;

        log::info!(
            "[SupjavProvider] 详情与视频流解析完成: title='{}', stream_url='{}'",
            title,
            stream_url
        );

        Ok(MediaDetail {
            id: detail_page_url.to_string(),
            title,
            cover_url,
            detail_page_url: detail_page_url.to_string(),
            stream_url,
            referer: Some(referer),
            categories,
            tags,
            actresses,
        })
    }
}

/// 解析 Streamtape / advtpe 等 JS 字符串拼接与 .substring(...) 链式截取表达式
fn parse_streamtape_expr(expr: &str) -> Option<String> {
    let mut result = String::new();
    let chars: Vec<char> = expr.chars().collect();
    let mut i = 0;
    let n = chars.len();

    while i < n {
        // 寻找开头的单引号或双引号
        if chars[i] == '\'' || chars[i] == '"' {
            let quote = chars[i];
            i += 1;
            let start = i;
            while i < n && chars[i] != quote {
                if chars[i] == '\\' && i + 1 < n {
                    i += 2;
                } else {
                    i += 1;
                }
            }
            let mut s: String = chars[start..i].iter().collect();
            if i < n && chars[i] == quote {
                i += 1;
            }

            // 检查后面是否有闭括号或紧随其后的 .substring(...) / .substr(...) / .slice(...)
            loop {
                // 跳过空白与可能包裹的括号，如 ('...').substring(2)
                while i < n && (chars[i].is_whitespace() || chars[i] == ')') {
                    i += 1;
                }
                let rest: String = chars[i..].iter().collect();
                if let Some(cap) = RE_SUBSTRING_CALL.captures(&rest) {
                    if let Some(full_match) = cap.get(0) {
                        let match_len = full_match.as_str().chars().count();
                        let p1: usize = cap
                            .get(1)
                            .and_then(|m| m.as_str().parse().ok())
                            .unwrap_or(0);
                        let p2: Option<usize> = cap.get(2).and_then(|m| m.as_str().parse().ok());

                        let s_len = s.chars().count();
                        let start_idx = p1.min(s_len);
                        let end_idx = p2.map(|e| e.min(s_len)).unwrap_or(s_len);
                        if start_idx <= end_idx {
                            s = s
                                .chars()
                                .skip(start_idx)
                                .take(end_idx - start_idx)
                                .collect();
                        } else {
                            s.clear();
                        }
                        i += match_len;
                        continue;
                    }
                }
                break;
            }

            result.push_str(&s);
        } else {
            i += 1;
        }
    }

    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

/// 从页面中提取 Streamtape / advtpe 相关的有效视频候选流（优先 stream=1）
fn extract_streamtape_urls(html: &str) -> Vec<String> {
    let mut urls = Vec::new();

    // 1. 如果 HTML 中已包含 tapecontent.net 展开地址，直接提取（排除海报缩略图）
    for cap in RE_TAPECONTENT.captures_iter(html) {
        if let Some(m) = cap.get(1) {
            let u = m.as_str().replace(r#"\/"#, "/");
            if !u.contains("thumb.tapecontent.net")
                && !u.ends_with(".jpg")
                && !u.ends_with(".png")
                && !u.ends_with(".webp")
                && !urls.contains(&u)
            {
                urls.push(u);
            }
        }
    }

    // 2. 匹配各类 innerHTML / 脚本拼接赋值语句（支持多行语句，如 robotlink, botlink, norobotlink, ideoolink）
    let mut cursor = 0;
    while let Some(idx) = html[cursor..].find(".innerHTML") {
        let abs_idx = cursor + idx + ".innerHTML".len();
        let rest = &html[abs_idx..];
        // 查找赋值等号 '='
        if let Some(eq_idx) = rest.find('=') {
            // 确保等号前只有空白字符（避免匹配到 == 或 !=）
            let between = &rest[..eq_idx].trim();
            if between.is_empty() {
                let expr_start = &rest[eq_idx + 1..];
                // 查找结束分号 ';'（限制最大跨度 500 字符）
                let expr = if let Some(semi_idx) = expr_start.find(';') {
                    &expr_start[..semi_idx]
                } else {
                    let end = expr_start.len().min(500);
                    &expr_start[..end]
                };

                if expr.contains("get_video") {
                    if let Some(mut evaluated) = parse_streamtape_expr(expr) {
                        evaluated = evaluated.trim().to_string();
                        if evaluated.contains("get_video")
                            && (evaluated.contains("?id=") || evaluated.contains("&id="))
                        {
                            let full_url = if evaluated.starts_with("//") {
                                format!("https:{}", evaluated)
                            } else if evaluated.starts_with('/') {
                                format!("https://advtpe.com{}", evaluated)
                            } else if !evaluated.starts_with("http://")
                                && !evaluated.starts_with("https://")
                            {
                                format!("https://{}", evaluated)
                            } else {
                                evaluated
                            };

                            // 优先生成带 &stream=1 的播放流直链（与浏览器行为完全一致）
                            let stream_url = if !full_url.contains("stream=1") {
                                if full_url.contains('?') {
                                    format!("{}&stream=1", full_url)
                                } else {
                                    format!("{}?stream=1", full_url)
                                }
                            } else {
                                full_url.clone()
                            };

                            if !urls.contains(&stream_url) {
                                urls.push(stream_url.clone());
                            }
                            if !urls.contains(&full_url) {
                                urls.push(full_url.clone());
                            }

                            // 如果是 streamtape.com，补充 advtpe.com 镜像候选；反之亦然
                            if full_url.contains("streamtape.com") {
                                let adv = stream_url.replace("streamtape.com", "advtpe.com");
                                if !urls.contains(&adv) {
                                    urls.push(adv);
                                }
                            } else if full_url.contains("advtpe.com") {
                                let st = stream_url.replace("advtpe.com", "streamtape.com");
                                if !urls.contains(&st) {
                                    urls.push(st);
                                }
                            }
                        }
                    }
                }
            }
        }
        cursor = abs_idx;
    }

    urls
}

/// 过滤非媒体流的常规网页地址（避免误把 /e/... 或 embed 播放器页面当作视频流）
fn is_web_page_url(url: &str) -> bool {
    let lower = url.to_lowercase();
    lower.contains("/e/")
        || lower.contains("/v/")
        || lower.contains("/embed/")
        || lower.contains("streamtape.com/e/")
        || lower.contains("advtpe.com/e/")
        || lower.contains(".html")
        || lower.contains(".php")
}

/// 从页面 HTML 中提取候选播放流地址（按优先级排序：专用网盘解密 > m3u8 > mp4 直链 > video/source 标签）
fn extract_stream_candidates(html: &str) -> Vec<String> {
    let clean_html = html.replace(r#"\/"#, "/");
    let mut candidates = Vec::new();

    // 1. Streamtape / advtpe 专用解析（优先级最高）
    let st_urls = extract_streamtape_urls(&clean_html);
    for u in st_urls {
        if !candidates.contains(&u) {
            candidates.push(u);
        }
    }

    // 2. 匹配常见的 m3u8 格式
    for re in [&RE_URL_PLAY, &RE_SOURCE_M3U8, &RE_FALLBACK_M3U8] {
        for cap in re.captures_iter(&clean_html) {
            if let Some(m) = cap.get(1) {
                let url = m.as_str().to_string();
                if !is_web_page_url(&url) && !candidates.contains(&url) {
                    candidates.push(url);
                }
            }
        }
    }

    // 3. 匹配 mp4 直链（含 ?stream=1），注意排除网页 URL
    for cap in RE_MP4_STREAM.captures_iter(&clean_html) {
        if let Some(m) = cap.get(1) {
            let url = m.as_str().to_string();
            if !is_web_page_url(&url) && !candidates.contains(&url) {
                candidates.push(url);
            }
        }
    }

    // 4. 从 <video src="..."> 和 <source src="..."> 提取
    let doc = Document::from(clean_html.as_str());
    for sel in ["video", "source"] {
        for node in doc.select(sel).iter() {
            if let Some(src) = node.attr("src").map(|s| s.trim().to_string()) {
                if !is_web_page_url(&src)
                    && (src.contains(".m3u8") || src.contains(".mp4") || src.contains("stream=1"))
                {
                    let full_src = if src.starts_with("//") {
                        format!("https:{}", src)
                    } else {
                        src
                    };
                    if !candidates.contains(&full_src) {
                        candidates.push(full_src);
                    }
                }
            }
        }
    }

    candidates
}

/// 解析线路列表的视频流（按顺序轮询各线路，支持第一跳网关以及第二跳嵌套 iframe，配合通用探活机制）
async fn resolve_server_stream(
    client: &HttpClient,
    data_links: &Vec<String>,
    page_ua: Option<&str>,
) -> Result<(String, String), ExtractorError> {
    let send_get = |url: &str, referer: &str| {
        let mut req = client
            .get(url)
            .header("Referer", referer)
            .timeout(Duration::from_secs(12));
        if let Some(ua) = page_ua {
            req = req.header("User-Agent", ua);
        }
        // 不传递当前 supjav.com 的 Cookie，避免跨域网关或播放器源引发异常拦截
        req
    };

    let total_links = data_links.len();
    log::info!(
        "[SupjavProvider] 开始解析线路视频流 (共 {} 条候选线路)...",
        total_links
    );

    for (idx, data_link) in data_links.iter().enumerate() {
        let line_num = idx + 1;
        // 构造第一跳网关 URL（兼容直接完整 URL 与前端倒序字符串）
        let first_hop_url = if data_link.starts_with("http://") || data_link.starts_with("https://")
        {
            data_link.clone()
        } else if data_link.starts_with("//") {
            format!("https:{}", data_link)
        } else if data_link.starts_with('/') {
            format!("https://lk1.supremejav.com{}", data_link)
        } else {
            let reversed_link: String = data_link.chars().rev().collect();
            format!("https://lk1.supremejav.com/supjav.php?c={}", reversed_link)
        };

        log::info!(
            "[SupjavProvider] 请求线路网关 [{}/{}]: {}",
            line_num,
            total_links,
            first_hop_url
        );

        let hop1_start = std::time::Instant::now();
        // 第一跳：请求网关
        let resp1 = match send_get(&first_hop_url, "https://supjav.com/").send().await {
            Ok(r) => r,
            Err(e) => {
                log::warn!(
                    "[SupjavProvider] 线路 [{}/{}] 第一层请求失败 (耗时 {:.2}s): {:?}，尝试下一线路",
                    line_num,
                    total_links,
                    hop1_start.elapsed().as_secs_f32(),
                    e
                );
                continue;
            }
        };

        let first_final_url = resp1.url().to_string();
        let status1 = resp1.status();
        let Ok(first_html) = resp1.text().await else {
            log::warn!(
                "[SupjavProvider] 线路 [{}/{}] 读取网关响应失败，尝试下一线路",
                line_num,
                total_links
            );
            continue;
        };

        log::debug!(
            "[SupjavProvider] 线路 [{}/{}] 网关响应 (status={}, 耗时 {:.2}s): {}",
            line_num,
            total_links,
            status1,
            hop1_start.elapsed().as_secs_f32(),
            first_final_url
        );

        // 如果第一跳返回 streamtape 页面，检查是否需要 fallback 到 advtpe
        let mut first_html_to_parse = first_html;
        let mut first_effective_url = first_final_url.clone();
        if (first_html_to_parse.trim().is_empty() || !first_html_to_parse.contains("get_video"))
            && first_final_url.contains("streamtape.com")
        {
            let advtpe_url = first_final_url.replace("streamtape.com", "advtpe.com");
            log::info!(
                "[SupjavProvider] 线路 [{}/{}] streamtape 未直接获取有效内容，尝试请求 advtpe: {}",
                line_num,
                total_links,
                advtpe_url
            );
            if let Ok(resp_adv) = send_get(&advtpe_url, "https://supjav.com/").send().await {
                if let Ok(adv_text) = resp_adv.text().await {
                    if !adv_text.trim().is_empty() {
                        first_html_to_parse = adv_text;
                        first_effective_url = advtpe_url;
                    }
                }
            }
        }

        // 检查第一层是否直接返回了有效媒体流，并逐个进行网络探活
        let first_candidates = extract_stream_candidates(&first_html_to_parse);
        for candidate in first_candidates {
            log::info!(
                "[SupjavProvider] 线路 [{}/{}] 第一层发现候选流，开始探活: {}",
                line_num,
                total_links,
                candidate
            );
            let probe_referer =
                if candidate.contains("advtpe.com") || candidate.contains("tapecontent.net") {
                    "https://advtpe.com/"
                } else if candidate.contains("streamtape.com") {
                    "https://streamtape.com/"
                } else {
                    &first_effective_url
                };

            if let Some(stream_url) =
                probe_stream_url(client, &candidate, Some(probe_referer), page_ua).await
            {
                let final_referer = if stream_url.contains("tapecontent.net")
                    || stream_url.contains("advtpe.com")
                {
                    "https://advtpe.com/".to_string()
                } else if stream_url.contains("streamtape.com") {
                    "https://streamtape.com/".to_string()
                } else {
                    first_effective_url.clone()
                };
                log::info!(
                    "[SupjavProvider] 线路 [{}/{}] 第一层探活成功，采用播放流: {} (referer={})",
                    line_num,
                    total_links,
                    stream_url,
                    final_referer
                );
                return Ok((final_referer, stream_url));
            } else {
                log::warn!(
                    "[SupjavProvider] 线路 [{}/{}] 第一层候选流探活未通过 (404/失效)，继续检查后续: {}",
                    line_num,
                    total_links,
                    candidate
                );
            }
        }

        // 从 HTML 中提取第二层 iframe 播放器地址（过滤 javascript: / about: 等无效 scheme）
        let second_hop_urls: Vec<String> = {
            let mut urls = Vec::new();
            let doc = Document::from(first_html_to_parse.as_str());
            for node in doc.select("iframe").iter() {
                if let Some(src) = node.attr("src").map(|s| s.trim().to_string()) {
                    if src.is_empty()
                        || src.starts_with("javascript:")
                        || src.starts_with("about:")
                        || src.starts_with('#')
                    {
                        continue;
                    }
                    let resolved = if src.starts_with("http://") || src.starts_with("https://") {
                        Some(src)
                    } else if src.starts_with("//") {
                        Some(format!("https:{}", src))
                    } else if let Ok(base) = url::Url::parse(&first_effective_url) {
                        base.join(&src).ok().map(|u| u.to_string())
                    } else {
                        None
                    };
                    if let Some(u) = resolved {
                        if !urls.contains(&u) {
                            urls.push(u);
                        }
                    }
                }
            }
            urls
        };

        if second_hop_urls.is_empty() {
            log::warn!(
                "[SupjavProvider] 线路 [{}/{}] 第一层无可用流且无嵌套有效 iframe",
                line_num,
                total_links
            );
            continue;
        }

        for second_hop_url in second_hop_urls {
            log::info!(
                "[SupjavProvider] 线路 [{}/{}] 发现嵌套 iframe，请求第二层播放器: {}",
                line_num,
                total_links,
                second_hop_url
            );

            let hop2_start = std::time::Instant::now();
            // 第二跳：请求播放器真实页面（Referer 设为第一跳地址）
            let resp2 = match send_get(&second_hop_url, &first_effective_url).send().await {
                Ok(r) => r,
                Err(e) => {
                    log::warn!(
                        "[SupjavProvider] 线路 [{}/{}] 第二层请求失败 (耗时 {:.2}s): {:?}，继续下一 iframe",
                        line_num,
                        total_links,
                        hop2_start.elapsed().as_secs_f32(),
                        e
                    );
                    continue;
                }
            };

            let second_final_url = resp2.url().to_string();
            let Ok(second_html) = resp2.text().await else {
                log::warn!(
                    "[SupjavProvider] 线路 [{}/{}] 读取第二层响应失败，继续下一 iframe",
                    line_num,
                    total_links
                );
                continue;
            };

            // 从第二层页面解析候选流并逐一探活
            let second_candidates = extract_stream_candidates(&second_html);
            for candidate in second_candidates {
                log::info!(
                    "[SupjavProvider] 线路 [{}/{}] 第二层发现候选流，开始探活: {}",
                    line_num,
                    total_links,
                    candidate
                );
                let probe_referer =
                    if candidate.contains("advtpe.com") || candidate.contains("tapecontent.net") {
                        "https://advtpe.com/"
                    } else if candidate.contains("streamtape.com") {
                        "https://streamtape.com/"
                    } else {
                        &second_final_url
                    };

                if let Some(stream_url) =
                    probe_stream_url(client, &candidate, Some(probe_referer), page_ua).await
                {
                    let final_referer = if stream_url.contains("tapecontent.net")
                        || stream_url.contains("advtpe.com")
                    {
                        "https://advtpe.com/".to_string()
                    } else if stream_url.contains("streamtape.com") {
                        "https://streamtape.com/".to_string()
                    } else {
                        second_final_url.clone()
                    };
                    log::info!(
                        "[SupjavProvider] 线路 [{}/{}] 第二层探活成功，采用播放流: {} (referer={})",
                        line_num,
                        total_links,
                        stream_url,
                        final_referer
                    );
                    return Ok((final_referer, stream_url));
                } else {
                    log::warn!(
                        "[SupjavProvider] 线路 [{}/{}] 第二层候选流探活未通过: {}",
                        line_num,
                        total_links,
                        candidate
                    );
                }
            }
        }

        log::warn!(
            "[SupjavProvider] 线路 [{}/{}] 所有候选流均未能通过探活验证，尝试下一线路",
            line_num,
            total_links
        );
    }

    log::error!(
        "[SupjavProvider] 全部 {} 条线路均未能提取到有效的媒体流地址",
        total_links
    );
    Err(ExtractorError::MediaNotFound)
}

fn get_supjav_quick_links(domain_clean: &str) -> Vec<NavItem> {
    let raw_quick_links = vec![
        ("popular", "popular"),
        ("weekly popular", "popular?sort=week"),
        ("monthly popular", "popular?sort=month"),
    ];
    raw_quick_links
        .into_iter()
        .map(|(name, path)| NavItem {
            name: name.to_string(),
            url: format!("{}/{}", domain_clean, path),
        })
        .collect()
}

fn get_supjav_categories(domain_clean: &str) -> Vec<NavItem> {
    let raw_categories = vec![
        ("censored jav", "censored-jav"),
        ("uncensored jav", "uncensored-jav"),
        ("amateur", "amateur"),
        ("chinese subtitles", "chinese-subtitles"),
        ("reducing mosaic", "reducing-mosaic"),
        ("english subtitles", "english-subtitles"),
    ];

    raw_categories
        .into_iter()
        .map(|(name, slug)| NavItem {
            name: name.to_string(),
            url: format!("{}/category/{}/", domain_clean, slug),
        })
        .collect()
}

fn get_supjav_tags(domain_clean: &str) -> Vec<TagGroup> {
    let raw_tags: Vec<(&str, Vec<(&str, &str)>)> = vec![
        (
            "clothing",
            vec![
                ("uniform", "uniform"),
                ("pantyhose", "pantyhose"),
                ("school uniform", "school-uniform"),
                ("swimsuit", "swimsuit"),
                ("lingerie", "lingerie"),
                ("school swimsuit", "school-swimsuit"),
                ("underwear", "underwear"),
                ("glasses", "glasses"),
                ("kimonomourning", "kimonomourning"),
                ("sailor suit", "sailor-suit"),
                ("bloomers", "bloomers"),
                ("mini skirt", "mini-skirt"),
                ("erotic wear", "erotic-wear"),
                ("long boots", "long-boots"),
                ("bunny girl", "bunny-girl"),
                ("yukata", "yukata"),
                ("leotard", "leotard"),
                ("business attire", "business-attire"),
                ("no bra", "no-bra"),
                ("knee socks", "knee-socks"),
                ("cosplay", "cosplay"),
                ("cross dressing", "cross-dressing"),
            ],
        ),
        (
            "body",
            vec![
                ("big tits", "big-tits"),
                ("slender", "slender"),
                ("breasts", "breasts"),
                ("shaved", "shaved"),
                ("huge butt", "huge-butt"),
                ("nice ass", "nice-ass"),
                ("butt", "butt"),
                ("tits", "tits"),
                ("sexy legs", "sexy-legs"),
                ("huge cock", "huge-cock"),
                ("bbw", "bbw"),
                ("tall", "tall"),
                ("ultra huge tits", "ultra-huge-tits"),
                ("mini", "mini"),
                ("sun tan", "sun-tan"),
                ("muscle", "muscle"),
                ("body conscious", "body-conscious"),
            ],
        ),
        (
            "acts",
            vec![
                ("creampie", "creampie"),
                ("blowjob", "blowjob"),
                ("squirting", "squirting"),
                ("titty fuck", "titty-fuck"),
                ("cowgirl", "cowgirl"),
                ("handjob", "handjob"),
                ("deep throating", "deep-throating"),
                ("humiliation", "humiliation"),
                ("masturbation", "masturbation"),
                ("facials", "facials"),
                ("anal", "anal"),
                ("cum", "cum"),
                ("finger fuck", "finger-fuck"),
                ("footjob", "footjob"),
                ("cumshot", "cumshot"),
            ],
        ),
        (
            "plays",
            vec![
                ("3p4p", "3p4p"),
                ("nastyhardcore", "nastyhardcore"),
                ("voyeur", "voyeur"),
                ("bukkake", "bukkake"),
                ("promiscuity", "promiscuity"),
                ("dirty words", "dirty-words"),
                ("restraint", "restraint"),
                ("lotion", "lotion"),
                ("toy", "toy"),
                ("acmeorgasm", "acmeorgasm"),
                ("massage", "massage"),
                ("electric massager", "electric-massager"),
                ("cunnilingus", "cunnilingus"),
                ("training", "training"),
                ("lesbian", "lesbian"),
                ("sweat", "sweat"),
                ("busty fetish", "busty-fetish"),
                ("sm", "sm"),
                ("vibe", "vibe"),
                ("leg fetish", "leg-fetish"),
                ("69", "69"),
                ("gangbang", "gangbang"),
                ("other fetish", "other-fetish"),
                ("confinement", "confinement"),
                ("impromptu sex", "impromptu-sex"),
                ("lesbian kiss", "lesbian-kiss"),
                ("molester", "molester"),
                ("facesitting", "facesitting"),
                ("back", "back"),
                ("bareback", "bareback"),
                ("male squirting", "male-squirting"),
                ("conceived", "conceived"),
                ("restraints", "restraints"),
                ("sport", "sport"),
                ("exposure", "exposure"),
                ("breast milk", "breast-milk"),
                ("dildo", "dildo"),
                ("riding facesitting", "riding-facesitting"),
                ("masturbation support", "masturbation-support"),
                ("bondage", "bondage"),
                ("egg vibrator", "egg-vibrator"),
                ("tsundere", "tsundere"),
                ("rolling back eyes fainting", "rolling-back-eyesfainting"),
                ("premature ejaculation", "premature-ejaculation"),
                ("fisting", "fisting"),
                ("dance", "dance"),
            ],
        ),
        (
            "theme",
            vec![
                ("prank", "prank"),
                ("solowork", "solowork"),
                ("planning", "planning"),
                ("drama", "drama"),
                ("documentary", "documentary"),
                ("debut production", "debut-production"),
                ("risky mosaic", "risky-mosaic"),
                ("bestomnibus", "bestomnibus"),
                ("subjectivity", "subjectivity"),
                ("pov", "pov"),
                ("close up", "close-up"),
                ("image video", "image-video"),
                ("couple", "couple"),
                ("original collaboration", "original-collaboration"),
                ("user submission", "user-submission"),
                ("love", "love"),
                ("multiple story", "multiple-story"),
                ("school stuff", "school-stuff"),
                ("fan appreciation", "fan-appreciation"),
                ("for women", "for-women"),
                ("affair", "affair"),
                ("cuckold", "cuckold"),
                ("fantasy", "fantasy"),
                ("time stop", "time-stop"),
                ("delusion", "delusion"),
                ("special effects", "special-effects"),
            ],
        ),
        (
            "character",
            vec![
                ("amateur", "amateur"),
                ("married woman", "married-woman"),
                ("mature woman", "mature-woman"),
                ("beautiful girl", "beautiful-girl"),
                ("slut", "slut"),
                ("older sister", "older-sister"),
                ("ol", "ol"),
                ("gal", "gal"),
                ("prostitutes", "prostitutes"),
                ("sister", "sister"),
                ("female teacher", "female-teacher"),
                ("various professions", "various-professions"),
                ("entertainer", "entertainer"),
                ("school girls", "school-girls"),
                ("brideyoung wife", "brideyoung-wife"),
                ("female college student", "female-college-student"),
                ("mother", "mother"),
                ("nurse", "nurse"),
                ("stepmother", "stepmother"),
                ("female boss", "female-boss"),
                ("black actor", "black-actor"),
                ("bitch", "bitch"),
                ("landladyhostess", "landladyhostess"),
                ("av actress", "av-actress"),
                ("female investigator", "female-investigator"),
                ("girl", "girl"),
                ("tutor", "tutor"),
                ("stewardess", "stewardess"),
                ("elder male", "elder-male"),
                ("widow", "widow"),
                ("secretary", "secretary"),
                ("white actress", "white-actress"),
                ("anchorwoman", "anchorwoman"),
                ("subordinatescolleagues", "subordinatescolleagues"),
                ("idol", "idol"),
                ("female doctor", "female-doctor"),
                ("model", "model"),
                ("instructor", "instructor"),
                ("race queen", "race-queen"),
                ("aunt", "aunt"),
                ("club activities manager", "club-activities-manager"),
                ("athlete", "athlete"),
                ("adopted daughter", "adopted-daughter"),
            ],
        ),
        (
            "scene",
            vec![
                ("outdoors", "outdoors"),
                ("beauty shop", "beauty-shop"),
                ("hot spring", "hot-spring"),
                ("soapland", "soapland"),
                ("hotel", "hotel"),
                ("travel", "travel"),
                ("car sex", "car-sex"),
                ("hospital clinic", "hospital-clinic"),
                ("date", "date"),
                ("bath", "bath"),
            ],
        ),
    ];

    raw_tags
        .into_iter()
        .map(|(group, tags)| TagGroup {
            group: group.to_string(),
            items: tags
                .into_iter()
                .map(|(name, slug)| NavItem {
                    name: name.to_string(),
                    url: format!("{}/tag/{}/", domain_clean, slug),
                })
                .collect(),
        })
        .collect()
}

fn get_supjav_sort_rules() -> Vec<SortRule> {
    vec![
        SortRule {
            match_patterns: vec!["?s=".to_string(), "&s=".to_string()],
            options: vec![],
        },
        SortRule {
            match_patterns: vec![
                "category/cast".to_string(),
                "category".to_string(),
                "tag".to_string(),
            ],
            options: vec![
                SortOption {
                    name: "most_viewed".to_string(),
                    value: "views".to_string(),
                },
                SortOption {
                    name: "this_month".to_string(),
                    value: "month".to_string(),
                },
            ],
        },
        SortRule {
            match_patterns: vec!["popular".to_string()],
            options: vec![
                SortOption {
                    name: "today".to_string(),
                    value: "day".to_string(),
                },
                SortOption {
                    name: "this_week".to_string(),
                    value: "week".to_string(),
                },
                SortOption {
                    name: "this_month".to_string(),
                    value: "month".to_string(),
                },
            ],
        },
    ]
}
