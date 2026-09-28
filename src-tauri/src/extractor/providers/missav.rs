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

const EXTRACT_READY_CONDIION_MISSAV: &str = r#"(document.querySelector('h1, meta[property="og:title"]') && document.documentElement.innerHTML.includes('m3u8'))"#;

const LIST_READY_CONDITION_MISSAV: &str = "document.querySelectorAll('div.thumbnail').length > 0";

static RE_SCRIPT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?s)<script[^>]*>(.*?)</script>"#).unwrap());
static RE_SOURCE_M3U8: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"source\s*=\s*[\\']*(https?://[^'\\;\s]+\.m3u8)"#).unwrap());
static RE_ANY_M3U8: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(https?://[^\'\\;\s]+\.m3u8)"#).unwrap());

pub struct MissavProvider {
    domains: Vec<String>,
    manifest: SiteManifest,
}

impl MissavProvider {
    // 默认内置域名
    pub fn default_domains() -> Vec<String> {
        vec![
            "https://missav.ws".to_string(),
            "https://missav.ai".to_string(),
            "https://missav.com".to_string(),
            "https://missav.live".to_string(),
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
            site: ProviderSite::Missav,
            name: "MissAV".to_string(),
            primary_domain: primary.to_string(),
            available_domains: domains.to_vec(),
            default_url: format!("{}/new", primary),
            supported_languages: HashMap::from([
                (Language::ZHTW, "zh".to_string()),
                (Language::ZHCN, "cn".to_string()),
                (Language::EN, "en".to_string()),
                (Language::JA, "ja".to_string()),
            ]),
            quick_links: get_missav_quick_links(primary),
            categories: get_missav_categories(primary),
            tags: get_missav_tags(primary),
            sort_rules: get_missav_sort_rules(),
        }
    }

    fn parse_video_card(node: &Selection) -> Option<VideoInfo> {
        let title_link = node.select("a.text-secondary");
        let title = Some(title_link.text().trim().to_string()).filter(|s| !s.is_empty())?;
        let detail_page_url = title_link
            .attr("href")
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())?;

        // 封面：img 的 data-src / src
        let img = node.select("img");
        // 缩略图
        let thumbnail_url = img
            .attr("data-src")
            .or_else(|| img.attr("src"))
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())?;
        // 高清图
        let cover_url = thumbnail_url
            .strip_suffix("-t.jpg")
            .map(|s| format!("{}-n.jpg", s))
            .unwrap_or(thumbnail_url);

        // 时长：右下角那个 span
        let duration = Some(
            node.select("span.absolute.bottom-1.right-1")
                .text()
                .trim()
                .to_string(),
        )
        .filter(|s| !s.is_empty());

        // 预览视频：video 的 data-src / src
        let video = node.select("video");
        let preview_url = video
            .attr("data-src")
            .or_else(|| video.attr("src"))
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty());

        Some(VideoInfo {
            id: detail_page_url.clone(),
            title,
            detail_page_url,
            cover_url,
            duration,
            preview_url,
            referer: None,
            ua: None,
        })
    }

    // missave 的参数为 /zh/new?page=10&sort=monthly_views
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

        if let Some(page) = params.page {
            query_map.insert("page".to_string(), page.to_string());
        }
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

        // 处理 path 中的 lang
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
        let is_dm = |s: &str| s.starts_with("dm");

        // 1. 删除 dmXXX 段
        segments.retain(|s| !is_dm(s));

        // 2. 删除已知语言段
        segments.retain(|s| !is_lang(s));

        // 3. 插入新语言到最前面
        if let Some(lang) = &params.lang {
            let code = lang_map
                .get(lang)
                .ok_or_else(|| ExtractorError::UnsupportedLanguage(*lang))?
                .clone();
            segments.insert(0, code);
        }

        Ok(format!("/{}", segments.join("/")))
    }
}

#[async_trait]
impl SiteProvider for MissavProvider {
    fn site(&self) -> ProviderSite {
        ProviderSite::Missav
    }

    fn domains(&self) -> &[String] {
        &self.domains
    }

    fn manifest(&self) -> &SiteManifest {
        &self.manifest
    }

    fn can_handle(&self, url: &str) -> bool {
        if url.contains("missav") {
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
        self.build_url(
            format!("{}/search/{}", primary_domain, encoded_keyword).as_str(),
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

        // 列表页就绪条件：出现至少一个视频封面卡片
        let page = webview
            .fetch_page(&url_with_params, LIST_READY_CONDITION_MISSAV)
            .await
            .map_err(|e| ExtractorError::Other(e.to_string()))?;

        let doc = Document::from(page.html.as_str());

        // '\n            / 2000\n        ' -> 2000
        let total_pages = doc
            .select("#price-currency")
            .first()
            .text()
            .split(|c: char| !c.is_ascii_digit())
            .find_map(|s| s.parse::<usize>().ok())
            .unwrap_or(1);

        log::debug!("[MissAV] Total pages {total_pages} of origin url {url_with_params}");
        log::debug!("[MissAV] The redirected/final url is {}", page.final_url);

        let mut videos: Vec<VideoInfo> = doc
            .select("div.thumbnail")
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
        _client: &HttpClient,
        detail_page_url: &str,
        params: Option<&FetchParams>,
    ) -> Result<MediaDetail, ExtractorError> {
        let url_with_params = self.build_url(detail_page_url, params)?;

        let html = webview
            .fetch_page(&url_with_params, EXTRACT_READY_CONDIION_MISSAV)
            .await
            .map_err(|e| ExtractorError::Other(e.to_string()))?;

        let doc = Document::from(html.html.as_str());

        let title = doc
            .select("meta[property=\"og:title\"]")
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

        let stream_url = RE_SCRIPT
            .captures_iter(&html.html)
            .filter_map(|cap| cap.get(1))
            .map(|m| m.as_str())
            .filter(|script| script.contains("eval(function") && script.contains("m3u8"))
            .filter_map(utils::unpack_js_eval)
            .find_map(|unpacked| {
                RE_SOURCE_M3U8
                    .captures(&unpacked)
                    .or_else(|| RE_ANY_M3U8.captures(&unpacked))
                    .and_then(|c| c.get(1))
                    .map(|m| m.as_str().to_string())
            })
            .ok_or(ExtractorError::MediaNotFound)?;

        let referer = html.final_url;
        let mut categories = Vec::new();
        let mut tags = Vec::new();
        let mut actresses = Vec::new();

        for el in doc.select("div.text-secondary").iter() {
            let items: Vec<NavItem> = el
                .select("a")
                .iter()
                .filter_map(|a| {
                    let name = a.text().trim().to_string();
                    let url = a.attr("href")?.trim().to_string();
                    (!name.is_empty() && !url.is_empty()).then_some(NavItem { name, url })
                })
                .collect();

            let has_genre = items.iter().any(|item| item.url.contains("genres"));

            for item in items {
                if item.url.contains("actresses") {
                    actresses.push(item);
                } else if item.url.contains("genres") {
                    tags.push(item);
                } else if has_genre {
                    // 与 tags(genres) 在同一个 div.text-secondary 下的不带 genres 的链接（如 /uncensored-leak）
                    categories.push(item);
                }
            }
        }

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

fn get_missav_sort_rules() -> Vec<SortRule> {
    vec![SortRule {
        match_patterns: vec!["*".to_string()],
        options: vec![
            SortOption {
                name: "released_at".to_string(),
                value: "released_at".to_string(),
            },
            SortOption {
                name: "latest".to_string(),
                value: "published_at".to_string(),
            },
            SortOption {
                name: "most_saved".to_string(),
                value: "saved".to_string(),
            },
            SortOption {
                name: "today".to_string(),
                value: "today_views".to_string(),
            },
            SortOption {
                name: "this_week".to_string(),
                value: "weekly_views".to_string(),
            },
            SortOption {
                name: "this_month".to_string(),
                value: "monthly_views".to_string(),
            },
            SortOption {
                name: "most_viewed".to_string(),
                value: "views".to_string(),
            },
        ],
    }]
}

fn get_missav_tags(domain_clean: &str) -> Vec<TagGroup> {
    let raw_tags: Vec<(&str, Vec<(&str, &str)>)> = vec![
        (
            "clothing",
            vec![
                ("maid", "Maid"),
                ("uniform", "Uniform"),
                ("pantyhose", "Pantyhose"),
                ("underwear", "Underwear"),
                ("swimsuit", "Swimsuit"),
                ("sailor suit", "Sailor%20Suit"),
                ("kimono", "Kimono"),
                ("short skirt", "Short%20Skirt"),
                ("sportswear", "Sportswear"),
                ("mini skirt", "Mini%20Skirt"),
                ("gym suit", "Gym%20Suit"),
                ("cosplay", "Cosplay"),
                ("glasses girl", "Glasses%20Girl"),
            ],
        ),
        (
            "body",
            vec![
                ("big breasts", "Big%20Breasts"),
                ("slim pixelated", "Slim%20Pixelated"),
                ("slim", "Slim"),
                ("beautiful breasts", "Beautiful%20Breasts"),
                ("shaving", "Shaving"),
                ("big ass", "Big%20Ass"),
                ("petite", "Petite"),
                ("small breasts", "Small%20Breasts"),
                ("ultra slim pixelated", "Ultra%20Slim%20Pixelated"),
                ("fat girl", "Fat%20Girl"),
                ("nice ass", "Nice%20Ass"),
                ("tall lady", "Tall%20Lady"),
                ("white skin", "White%20Skin"),
                ("beautiful legs", "Beautiful%20Legs"),
                ("super breasts", "Super%20Breasts"),
                ("black hair", "Black%20Hair"),
                ("bronze", "Bronze"),
            ],
        ),
        (
            "acts",
            vec![
                ("creampie", "Creampie"),
                ("tit job", "Tit%20Job"),
                ("squirting", "Squirting"),
                ("footjob", "Footjob"),
                ("ride", "Ride"),
                ("hit on girls", "Hit%20On%20Girls"),
                ("masturbate", "Masturbate"),
                ("cunnilingus", "Cunnilingus"),
                ("forced blowjob", "Forced%20Blowjob"),
                ("oral sex", "Oral%20Sex"),
                ("anal sex", "Anal%20Sex"),
                ("urinate", "Urinate"),
                ("doggy style", "Doggy%20Style"),
                ("humiliation", "Humiliation"),
            ],
        ),
        (
            "plays",
            vec![
                ("rejuvenation massage", "Rejuvenation%20Massage"),
                ("orgy", "Orgy"),
                ("bukkake", "Bukkake"),
                ("restraint", "Restraint"),
                ("adultery", "Adultery"),
                ("delusion", "Delusion"),
                ("lesbian", "Lesbian"),
                ("vibrator", "Vibrator"),
                ("fingering", "Fingering"),
                ("sm", "Sm"),
                ("promiscuity", "Promiscuity"),
                ("toy", "Toy"),
                ("dirty talk", "Dirty%20Talk"),
                ("massage", "Massage"),
                ("massage oil", "Massage%20Oil"),
                ("swallow sperm", "Swallow%20Sperm"),
                ("group bukkake", "Group%20Bukkake"),
                ("69", "69"),
                ("foot fetish", "Foot%20Fetish"),
                ("extreme orgasm", "Extreme%20Orgasm"),
                ("tied up", "Tied%20Up"),
                ("big breast fetish", "Big%20Breast%20Fetish"),
                ("sweating", "Sweating"),
                ("face ride", "Face%20Ride"),
                ("lesbian kiss", "Lesbian%20Kiss"),
                ("imprisonment", "Imprisonment"),
                ("vibrating egg", "Vibrating%20Egg"),
                ("mischief", "Mischief"),
                ("cruel", "Cruel"),
                ("torture", "Torture"),
                ("pure", "Pure"),
                ("instant sex", "Instant%20Sex"),
                ("3p, 4p", "3P,%204P"),
            ],
        ),
        (
            "theme",
            vec![
                ("individual", "Individual"),
                ("ntr", "Ntr"),
                ("incest", "Incest"),
                ("selfie", "Selfie"),
                ("plot", "Plot"),
                ("documentary", "Documentary"),
                ("subjective perspective", "Subjective%20Perspective"),
                ("delivery only", "Delivery%20Only"),
                ("debut", "Debut"),
                ("actress collection", "Actress%20Collection"),
                ("science fiction", "Science%20Fiction"),
                ("harem", "Harem"),
                ("multiple stories", "Multiple%20Stories"),
                ("campus story", "Campus%20Story"),
                ("in love", "In%20Love"),
                ("fantasy", "Fantasy"),
                ("thanks offering", "Thanks%20Offering"),
                ("original", "Original"),
                ("best, omnibus", "Best,%20Omnibus"),
                ("aphrodisiac", "Aphrodisiac"),
            ],
        ),
        (
            "character",
            vec![
                ("wife", "Wife"),
                ("young wife", "Young%20Wife"),
                ("mature woman", "Mature%20Woman"),
                ("ordinary person", "Ordinary%20Person"),
                ("pretty girl", "Pretty%20Girl"),
                ("slut", "Slut"),
                ("couple", "Couple"),
                ("high school girl", "High%20School%20Girl"),
                ("sister", "Sister"),
                ("ol", "Ol"),
                ("hot girl", "Hot%20Girl"),
                ("female college student", "Female%20College%20Student"),
                ("prostitute", "Prostitute"),
                ("mother", "Mother"),
                ("female teacher", "Female%20Teacher"),
                ("elder sister", "Elder%20Sister"),
                ("nurse", "Nurse"),
                ("virgin", "Virgin"),
                ("artist", "Artist"),
                ("married woman", "Married%20Woman"),
                ("stepmother", "Stepmother"),
                ("advertising idol", "Advertising%20Idol"),
                ("black male actor", "Black%20Male%20Actor"),
                ("female warrior", "Female%20Warrior"),
                ("transgender", "Transgender"),
                ("private teacher", "Private%20Teacher"),
                ("m female", "M%20Female"),
                ("widow", "Widow"),
                ("transsexuals", "Transsexuals"),
                ("bitch", "Bitch"),
                ("female doctor", "Female%20Doctor"),
                ("flight attendant", "Flight%20Attendant"),
                ("whites", "Whites"),
                ("missy", "Missy"),
                ("female boss", "Female%20Boss"),
                ("female investigator", "Female%20Investigator"),
                ("fighter", "Fighter"),
                ("foreign actress", "Foreign%20Actress"),
                ("delivery-only amateur", "Delivery-Only%20Amateur"),
                ("model", "Model"),
                ("various occupations", "Various%20Occupations"),
            ],
        ),
        (
            "location",
            vec![
                ("car sex", "Car%20Sex"),
                ("hot spring", "Hot%20Spring"),
                ("outdoor exposure", "Outdoor%20Exposure"),
                ("bubble bath", "Bubble%20Bath"),
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
                    url: format!("{}/genres/{}", domain_clean, slug),
                })
                .collect(),
        })
        .collect()
}

fn get_missav_categories(domain_clean: &str) -> Vec<NavItem> {
    let raw_categories = vec![
        ("uncensored leak", "uncensored-leak"),
        ("siro", "siro"),
        ("luxu", "luxu"),
        ("gana", "gana"),
        ("prestige premium", "maan"),
        ("ara", "ara"),
        ("fc2", "fc2"),
        ("heyzo", "heyzo"),
        ("tokyohot", "tokyohot"),
        ("1pondo", "1pondo"),
        ("madou", "madou"),
    ];

    raw_categories
        .into_iter()
        .map(|(name, path)| NavItem {
            name: name.to_string(),
            url: format!("{}/{}", domain_clean, path),
        })
        .collect()
}

fn get_missav_quick_links(domain_clean: &str) -> Vec<NavItem> {
    let raw_quick_links = vec![
        ("recent updates", "new"),
        ("today hot", "today-hot"),
        ("weekly hot", "weekly-hot"),
        ("monthly hot", "monthly-hot"),
    ];

    raw_quick_links
        .into_iter()
        .map(|(name, path)| NavItem {
            name: name.to_string(),
            url: format!("{}/{}", domain_clean, path),
        })
        .collect()
}
