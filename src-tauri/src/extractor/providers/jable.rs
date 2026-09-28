use std::collections::HashMap;
use std::sync::LazyLock;

use async_trait::async_trait;
use dom_query::{Document, Selection};
use regex::Regex;

use super::SiteProvider;
use crate::config::Language;
use crate::extractor::error::ExtractorError;
use crate::extractor::model::{
    FetchParams, MediaDetail, NavItem, ProviderSite, SiteManifest, SortOption, SortRule, TagGroup,
    VideoInfo, VideoPage,
};
use crate::network::{HttpClient, WebviewEngine};
use crate::utils;

const EXTRACT_READY_CONDITION_JABLE: &str = r#"window.hlsUrl || (document.querySelector('meta[property="og:title"]') && document.body && document.body.innerHTML.includes('hlsUrl'))"#;
const LIST_READY_CONDITION_JABLE: &str =
    "document.querySelector('#list_videos_latest_videos_list > div > section > ul, ul.pagination') || document.querySelectorAll('div.video-img-box').length >= 10 || (document.readyState === 'complete' && document.querySelectorAll('div.video-img-box').length > 0)";
static RE_HLS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"hlsUrl\s*=\s*['"]([^'"]+)['"]"#).unwrap());

pub struct JableProvider {
    domains: Vec<String>,
    manifest: SiteManifest,
}

impl JableProvider {
    // 默认内置域名
    pub fn default_domains() -> Vec<String> {
        vec![
            "https://jable.tv".to_string(),
            "https://fs1.app".to_string(),
        ]
    }

    // 可配置域名
    pub fn new(domains: Vec<String>) -> Self {
        let domains = if domains.is_empty() {
            Self::default_domains()
        } else {
            domains
        };
        let manifest = Self::build_manifest(&domains);
        Self { domains, manifest }
    }

    fn build_manifest(domains: &[String]) -> SiteManifest {
        let primary = domains[0].trim_end_matches('/');
        SiteManifest {
            site: ProviderSite::Jable,
            name: "Jable".to_string(),
            primary_domain: primary.to_string(),
            available_domains: domains.to_vec(),
            default_url: format!("{}/latest-updates/", primary),
            quick_links: get_jable_quick_links(primary),
            categories: get_jable_categories(primary),
            tags: get_jable_tags(primary),
            supported_languages: HashMap::from([
                (Language::EN, "en".to_string()),
                (Language::JA, "jp".to_string()),
                (Language::ZHTW, "zh".to_string()),
                (Language::ZHCN, "zh".to_string()),
            ]),
            sort_rules: get_jable_sort_rules(),
        }
    }

    // jable 的参数为 ?page=11&lang=cn&sort_by=post_date
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

        let mut query_map: std::collections::BTreeMap<String, String> = url
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.trim_end_matches('/').to_string()))
            .collect();

        if let Some(page) = params.page {
            query_map.insert("from".to_string(), page.to_string());
        }
        if let Some(sort_by) = &params.sort_by {
            if !sort_by.is_empty() {
                query_map.insert("sort_by".to_string(), sort_by.clone());
            }
        }
        if let Some(lang) = params.lang {
            let code = self
                .manifest
                .supported_languages
                .get(&lang)
                .ok_or_else(|| ExtractorError::UnsupportedLanguage(lang))?;
            query_map.insert("lang".to_string(), code.to_string());
        }

        url.set_query(None);
        if !query_map.is_empty() {
            let mut qp = url.query_pairs_mut();
            for (k, v) in &query_map {
                qp.append_pair(k, v);
            }
        }

        Ok(url.to_string())
    }

    fn parse_video_card(node: &Selection) -> Option<VideoInfo> {
        let detail = node.select(".detail a");
        let image = node.select(".img-box a");

        let title = Some(detail.text().trim().to_string()).filter(|s| !s.is_empty())?;
        let detail_page_url = detail
            .attr("href")
            .or_else(|| image.attr("href"))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())?;

        let img = image.select("img");
        // 缩略图
        let thumbnail_url = img
            .attr("data-src")
            .or_else(|| img.attr("src"))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())?;
        // 高清图
        let cover_url = match thumbnail_url.rfind('/') {
            Some(last) => match thumbnail_url[..last].rfind('/') {
                Some(second_last) => format!("{}/preview.jpg", &thumbnail_url[..second_last]),
                None => thumbnail_url.clone(),
            },
            None => thumbnail_url.clone(),
        };

        let preview_url = img.attr("data-preview").map(|s| s.trim().to_string());

        let duration =
            Some(image.select("span.label").text().trim().to_string()).filter(|s| !s.is_empty());

        Some(VideoInfo {
            id: detail_page_url.clone(),
            title,
            cover_url,
            detail_page_url,
            duration,
            preview_url,
            referer: None,
            ua: None,
        })
    }
}

#[async_trait]
impl SiteProvider for JableProvider {
    fn site(&self) -> ProviderSite {
        ProviderSite::Jable
    }

    fn domains(&self) -> &[String] {
        &self.domains
    }

    fn manifest(&self) -> &SiteManifest {
        &self.manifest
    }

    fn can_handle(&self, url: &str) -> bool {
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
        let primary = self.primary_domain();
        self.build_url(
            format!("{}/search/{}/", primary, encoded_keyword).as_str(),
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
            .fetch_page(&url_with_params, LIST_READY_CONDITION_JABLE)
            .await
            .map_err(|e| ExtractorError::Other(e.to_string()))?;
        let doc = Document::from(page.html.as_str());

        // '22261 部影片' -> 22261
        let total_pages = doc
            .select("div.title-box span")
            .first()
            .text()
            .split(|c: char| !c.is_ascii_digit())
            .find_map(|s| s.parse::<usize>().ok())
            .map(|s| s.div_ceil(24))
            .unwrap_or(1);

        log::debug!("[Jable] Total pages {total_pages} of origin url {url_with_params}");
        log::debug!("[Jable] The redirected/final url is {}", page.final_url);

        let mut videos: Vec<VideoInfo> = doc
            .select("div.video-img-box")
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
            total_pages: total_pages,
        })
    }

    async fn extract(
        &self,
        webview: &WebviewEngine,
        _client: &HttpClient,
        detail_page_url: &str,
        params: Option<&FetchParams>,
    ) -> Result<MediaDetail, ExtractorError> {
        let url_with_params = self.build_url(detail_page_url, params)?;

        let page = webview
            .fetch_page(&url_with_params, EXTRACT_READY_CONDITION_JABLE)
            .await
            .map_err(|e| {
                ExtractorError::HtmlGetFailed(url_with_params.to_string(), e.to_string())
            })?;

        let doc = Document::from(page.html.as_str());

        let title = doc
            .select(r#"meta[property="og:title"]"#)
            .attr("content")
            .map(|v| v.trim().to_string())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                ExtractorError::HtmlParse("title".to_string(), url_with_params.to_string())
            })?;

        let cover_url = doc
            .select("meta[property=\"og:image\"]")
            .attr("content")
            .map(|v| v.to_string())
            .ok_or_else(|| {
                ExtractorError::HtmlParse("cover_url".to_string(), url_with_params.to_string())
            })?;

        let stream_url = RE_HLS
            .captures(&page.html)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .ok_or(ExtractorError::MediaNotFound)?;

        let categories: Vec<NavItem> = doc
            .select("h5 a.cat, .tags a.cat, h5 a[href*='/categories/']")
            .iter()
            .filter_map(|node| {
                let name = node.text().trim().to_string();
                let href = node.attr("href")?.trim().to_string();
                (!name.is_empty() && !href.is_empty()).then_some(NavItem { name, url: href })
            })
            .collect();

        let tags: Vec<NavItem> = doc
            .select("h5 a[href*='/tags/'], .tags a:not(.cat), h5 a:not(.cat)")
            .iter()
            .filter_map(|node| {
                let name = node.text().trim().to_string();
                let href = node.attr("href")?.trim().to_string();
                (!name.is_empty() && !href.is_empty()).then_some(NavItem { name, url: href })
            })
            .collect();

        let actresses: Vec<NavItem> = doc
            .select(".models a.model, .models a")
            .iter()
            .filter_map(|node| {
                let url = node
                    .attr("href")
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())?;

                let target = node.select("img, span");
                let name = target
                    .attr("data-original-title")
                    .or_else(|| target.attr("title"))
                    .or_else(|| target.attr("alt"))
                    .or_else(|| node.attr("data-original-title"))
                    .or_else(|| node.attr("title"))
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())?;

                Some(NavItem { name, url })
            })
            .collect();

        let referer = page.final_url;

        Ok(MediaDetail {
            id: detail_page_url.to_string(),
            title,
            cover_url: cover_url,
            detail_page_url: detail_page_url.to_string(),
            stream_url,
            referer: Some(referer),
            categories,
            tags,
            actresses,
        })
    }
}

fn get_jable_sort_rules() -> Vec<SortRule> {
    vec![
        SortRule {
            match_patterns: vec![
                "categories".to_string(),
                "tags".to_string(),
                "search".to_string(),
                "models".to_string(),
            ],
            options: vec![
                SortOption {
                    name: "recent_best".to_string(),
                    value: "post_date_and_popularity".to_string(),
                },
                SortOption {
                    name: "latest".to_string(),
                    value: "post_date".to_string(),
                },
                SortOption {
                    name: "most_viewed".to_string(),
                    value: "video_viewed".to_string(),
                },
                SortOption {
                    name: "most_favourited".to_string(),
                    value: "most_favourited".to_string(),
                },
            ],
        },
        SortRule {
            match_patterns: vec!["hot".to_string()],
            options: vec![
                SortOption {
                    name: "all_time".to_string(),
                    value: "video_viewed".to_string(),
                },
                SortOption {
                    name: "today".to_string(),
                    value: "video_viewed_today".to_string(),
                },
                SortOption {
                    name: "this_week".to_string(),
                    value: "video_viewed_week".to_string(),
                },
                SortOption {
                    name: "this_month".to_string(),
                    value: "video_viewed_month".to_string(),
                },
            ],
        },
    ]
}

fn get_jable_quick_links(domain_clean: &str) -> Vec<NavItem> {
    let raw_quick_links = vec![
        ("latest updates", "latest-updates"),
        ("new release", "new-release"),
        ("today hot", "hot/?sort_by=video_viewed_today"),
        ("weekly hot", "hot/?sort_by=video_viewed_week"),
        ("monthly hot", "hot/?sort_by=video_viewed_month"),
        ("all time hot", "hot/?sort_by=video_viewed"),
    ];

    raw_quick_links
        .into_iter()
        .map(|(name, slug)| NavItem {
            name: name.to_string(),
            url: format!("{}/{}/", domain_clean, slug),
        })
        .collect()
}

fn get_jable_categories(domain_clean: &str) -> Vec<NavItem> {
    let raw_categories = vec![
        ("bdsm", "bdsm"),
        ("sex only", "sex-only"),
        ("chinese subtitles", "chinese-subtitle"),
        ("insult", "insult"),
        ("uniform", "uniform"),
        ("roleplay", "roleplay"),
        ("private cam", "private-cam"),
        ("uncensored", "uncensored"),
        ("pov", "pov"),
        ("group sex", "groupsex"),
        ("pantyhose", "pantyhose"),
        ("lesbian", "lesbian"),
    ];

    raw_categories
        .into_iter()
        .map(|(name, slug)| NavItem {
            name: name.to_string(),
            url: format!("{}/categories/{}/", domain_clean, slug),
        })
        .collect()
}

fn get_jable_tags(domain_clean: &str) -> Vec<TagGroup> {
    let raw_tags: Vec<(&str, Vec<(&str, &str)>)> = vec![
        (
            "clothing",
            vec![
                ("wedding dress", "wedding-dress"),
                ("swimsuit", "swimsuit"),
                ("stockings", "stockings"),
                ("sportswear", "sportswear"),
                ("school uniform", "school-uniform"),
                ("pantyhose", "pantyhose"),
                ("maid", "maid"),
                ("knee socks", "knee-socks"),
                ("kimono", "kimono"),
                ("kemonomimi", "kemonomimi"),
                ("glasses", "glasses"),
                ("flesh toned pantyhose", "flesh-toned-pantyhose"),
                ("fishnets", "fishnets"),
                ("cheongsam", "cheongsam"),
                ("bunny girl", "bunny-girl"),
                ("black pantyhose", "black-pantyhose"),
                ("cosplay", "Cosplay"),
            ],
        ),
        (
            "body",
            vec![
                ("big tits", "big-tits"),
                ("suntan", "suntan"),
                ("tall", "tall"),
                ("flexible body", "flexible-body"),
                ("small tits", "small-tits"),
                ("beautiful leg", "beautiful-leg"),
                ("beautiful butt", "beautiful-butt"),
                ("tattoo", "tattoo"),
                ("short hair", "short-hair"),
                ("hairless pussy", "hairless-pussy"),
                ("mature woman", "mature-woman"),
                ("girl", "girl"),
                ("dainty", "dainty"),
            ],
        ),
        (
            "acts",
            vec![
                ("tit wank", "tit-wank"),
                ("squirting", "squirting"),
                ("spasms", "spasms"),
                ("footjob", "footjob"),
                ("facial", "facial"),
                ("deep throat", "deep-throat"),
                ("cum in mouth", "cum-in-mouth"),
                ("creampie", "creampie"),
                ("blowjob", "blowjob"),
                ("anal sex", "anal-sex"),
            ],
        ),
        (
            "plays",
            vec![
                ("massage", "massage"),
                ("gang intrusion", "gang-intrusion"),
                ("intrusion", "intrusion"),
                ("tune", "tune"),
                ("bondage", "bondage"),
                ("chikan", "chikan"),
                ("chizyo", "chizyo"),
                ("masochism guy", "masochism-guy"),
                ("crapulence", "crapulence"),
                ("soapland", "soapland"),
                ("breast milk", "breast-milk"),
                ("piss", "piss"),
                ("grip", "grip"),
                ("3p", "3p"),
                ("10 times a day", "10-times-a-day"),
            ],
        ),
        (
            "theme",
            vec![
                ("virginity", "virginity"),
                ("time stop", "time-stop"),
                ("temptation", "temptation"),
                ("sex beside husband", "sex-beside-husband"),
                ("rainy day", "rainy-day"),
                ("ntr", "ntr"),
                ("love potion", "love-potion"),
                ("leakage", "private-cam"),
                ("kinship", "kinship"),
                ("hypnosis", "hypnosis"),
                ("giant man", "giant"),
                ("black", "black"),
                ("avenge", "avenge"),
                ("age difference", "age-difference"),
                ("affair", "affair"),
            ],
        ),
        (
            "character",
            vec![
                ("wife", "wife"),
                ("widow", "widow"),
                ("teen manager", "teen-manager"),
                ("teacher", "teacher"),
                ("sex worker", "club-hostess-and-sex-worker"),
                ("private teacher", "private-teacher"),
                ("ol", "ol"),
                ("nurse", "nurse"),
                ("idol", "idol"),
                ("housewife", "housewife"),
                ("fugitive", "fugitive"),
                ("flight attendant", "flight-attendant"),
                ("female anchor", "female anchor"),
                ("doctor", "doctor"),
                ("detective", "detective"),
                ("couple", "couple"),
            ],
        ),
        (
            "location",
            vec![
                ("outdoor", "outdoor"),
                ("tram", "tram"),
                ("toilet", "toilet"),
                ("swimming pool", "swimming-pool"),
                ("store", "store"),
                ("school", "school"),
                ("prison", "prison"),
                ("magic mirror", "magic-mirror"),
                ("library", "library"),
                ("hot spring", "hot-spring"),
                ("gym room", "gym-room"),
                ("first night", "first-night"),
                ("car", "car"),
                ("bathing place", "bathing-place"),
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
                    url: format!("{}/tags/{}/", domain_clean, slug),
                })
                .collect(),
        })
        .collect()
}
