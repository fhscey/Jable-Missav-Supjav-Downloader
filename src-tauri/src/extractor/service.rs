use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

use super::error::ExtractorError;
use super::model::{FetchParams, MediaDetail, ProviderSite, SiteManifest, VideoPage};
use super::providers::{JableProvider, MissavProvider, SupjavProvider};
use crate::config::AppConfig;
use crate::extractor::providers::SiteProvider;
use crate::network::{HttpClient, WebviewEngine};
use crate::utils::url::replace_url_domain;

/// 数据解析服务，多域名容灾
pub struct ExtractService {
    http_client: Arc<RwLock<HttpClient>>,
    webview: WebviewEngine,
    providers: parking_lot::RwLock<HashMap<ProviderSite, Arc<dyn SiteProvider>>>,
}

impl ExtractService {
    pub fn new(
        http_client: Arc<RwLock<HttpClient>>,
        webview: WebviewEngine,
        config: Arc<RwLock<AppConfig>>,
    ) -> Self {
        let (jable_domains, missav_domains, supjav_domains) = {
            let cfg = config.read();
            (
                cfg.site_domains
                    .get(&ProviderSite::Jable)
                    .cloned()
                    .unwrap_or_default(),
                cfg.site_domains
                    .get(&ProviderSite::Missav)
                    .cloned()
                    .unwrap_or_default(),
                cfg.site_domains
                    .get(&ProviderSite::Supjav)
                    .cloned()
                    .unwrap_or_default(),
            )
        };

        let mut providers: HashMap<ProviderSite, Arc<dyn SiteProvider>> = HashMap::new();
        providers.insert(
            ProviderSite::Jable,
            Arc::new(JableProvider::new(jable_domains)),
        );
        providers.insert(
            ProviderSite::Missav,
            Arc::new(MissavProvider::new(missav_domains)),
        );
        providers.insert(
            ProviderSite::Supjav,
            Arc::new(SupjavProvider::new(supjav_domains)),
        );

        Self {
            http_client,
            webview,
            providers: parking_lot::RwLock::new(providers),
        }
    }

    pub fn update_site_domains(
        &self,
        site: ProviderSite,
        domains: Vec<String>,
    ) -> Result<(), ExtractorError> {
        let new_provider: Arc<dyn SiteProvider> = match site {
            ProviderSite::Jable => Arc::new(JableProvider::new(domains)),
            ProviderSite::Missav => Arc::new(MissavProvider::new(domains)),
            ProviderSite::Supjav => Arc::new(SupjavProvider::new(domains)),
        };
        self.providers.write().insert(site, new_provider);
        Ok(())
    }

    pub fn get_all_site_manifests(&self) -> Vec<SiteManifest> {
        let providers = self.providers.read();
        let sites = [
            ProviderSite::Jable,
            ProviderSite::Missav,
            ProviderSite::Supjav,
        ];
        sites
            .iter()
            .filter_map(|s| providers.get(s))
            .map(|p| p.manifest().clone())
            .collect()
    }

    fn find_provider(&self, clean_url: &str) -> Result<Arc<dyn SiteProvider>, ExtractorError> {
        self.providers
            .read()
            .values()
            .find(|p| p.can_handle(clean_url))
            .cloned()
            .ok_or_else(|| {
                ExtractorError::UnsupportedUrl(format!(
                    "No media provider found for URL: {}",
                    clean_url
                ))
            })
    }

    fn get_provider(&self, site: ProviderSite) -> Result<Arc<dyn SiteProvider>, ExtractorError> {
        self.providers
            .read()
            .get(&site)
            .cloned()
            .ok_or(ExtractorError::UnsupportedSite(site))
    }

    pub async fn extract(
        &self,
        url: &str,
        params: Option<&FetchParams>,
    ) -> Result<MediaDetail, ExtractorError> {
        let clean_url = url.trim();
        let provider = self.find_provider(clean_url)?;
        let urls = candidate_urls(clean_url, provider.domains());
        let client = self.http_client.read().clone();

        run_with_failover(&urls, "extract", |u| {
            provider.extract(&self.webview, &client, u, params)
        })
        .await
    }

    pub async fn list_videos(
        &self,
        url: &str,
        params: Option<&FetchParams>,
    ) -> Result<VideoPage, ExtractorError> {
        let clean_url = url.trim();
        let provider = self.find_provider(clean_url)?;
        let urls = candidate_urls(clean_url, provider.domains());

        run_with_failover(&urls, "fetch_videos", |u| {
            provider.list(&self.webview, u, params)
        })
        .await
    }

    pub async fn search(
        &self,
        site: ProviderSite,
        keyword: &str,
        params: Option<&FetchParams>,
    ) -> Result<VideoPage, ExtractorError> {
        let provider = self.get_provider(site)?;
        let search_url = provider.search_url(keyword, params).ok_or_else(|| {
            ExtractorError::UnsupportedUrl(format!("Search not supported for {site}"))
        })?;

        self.list_videos(&search_url, params).await
    }
}

/// 生成候选 URL 列表（首项为原始 URL，随后依次为候选备用域名替换后的有效 URL）
fn candidate_urls(raw_url: &str, domains: &[String]) -> Vec<String> {
    let clean = raw_url.trim();
    let mut urls = vec![clean.to_string()];
    for d in domains {
        if let Some(candidate) = replace_url_domain(clean, d) {
            if candidate != clean && !urls.contains(&candidate) {
                urls.push(candidate);
            }
        }
    }
    urls
}

/// 泛型故障切换重试执行器：依次尝试各候选目标，首次成功即返回，全部失败返回末次错误
async fn run_with_failover<T, I, F, Fut>(
    candidates: I,
    action_name: &str,
    mut op: F,
) -> Result<T, ExtractorError>
where
    I: IntoIterator,
    F: FnMut(I::Item) -> Fut,
    Fut: std::future::Future<Output = Result<T, ExtractorError>>,
{
    let mut last_err = None;
    for item in candidates {
        match op(item).await {
            Ok(val) => return Ok(val),
            Err(e) => {
                if !e.is_failover_eligible() {
                    log::info!(
                        "[ExtractorService] {} 遇到参数或非网络错误 (err={:?})，直接返回不进行候选切换",
                        action_name,
                        e
                    );
                    return Err(e);
                }
                log::warn!(
                    "[ExtractorService] {} 尝试失败: err={:?}, 准备切换候选重试",
                    action_name,
                    e
                );
                last_err = Some(e);
            }
        }
    }
    Err(last_err.unwrap_or_else(|| {
        ExtractorError::Other(format!("No candidates available for {}", action_name))
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Language;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_replace_url_domain() {
        let raw = "https://jable.tv/videos/ssis-001/?lang=zh";
        let target = "https://fs1.app";
        let replaced = replace_url_domain(raw, target).unwrap();
        assert_eq!(replaced, "https://fs1.app/videos/ssis-001/?lang=zh");

        let raw_missav = "https://missav.ws/dm132/ssis-001";
        let target_missav = "https://missav.ai";
        let replaced_missav = replace_url_domain(raw_missav, target_missav).unwrap();
        assert_eq!(replaced_missav, "https://missav.ai/dm132/ssis-001");
    }

    #[test]
    fn test_replace_url_domain_with_port() {
        let raw = "http://localhost:3000/test/path";
        let target = "https://example.com";
        let replaced = replace_url_domain(raw, target).unwrap();
        assert_eq!(replaced, "https://example.com/test/path");
    }

    #[tokio::test]
    async fn test_run_with_failover_unsupported_language_fails_fast() {
        let candidates = vec!["domain1".to_string(), "domain2".to_string()];
        let attempts = Arc::new(AtomicUsize::new(0));
        let attempts_clone = Arc::clone(&attempts);

        let result = run_with_failover(candidates, "fetch_videos", move |_item| {
            let attempts = Arc::clone(&attempts_clone);
            async move {
                attempts.fetch_add(1, Ordering::SeqCst);
                Err::<(), _>(ExtractorError::UnsupportedLanguage(Language::ZHCN))
            }
        })
        .await;

        assert!(matches!(result, Err(ExtractorError::UnsupportedLanguage(Language::ZHCN))));
        // 遇到 UnsupportedLanguage 参数错误时应当立即终止，尝试次数为 1，不切换候选
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_run_with_failover_timeout_switches_candidate() {
        let candidates = vec!["domain1".to_string(), "domain2".to_string()];
        let attempts = Arc::new(AtomicUsize::new(0));
        let attempts_clone = Arc::clone(&attempts);

        let result = run_with_failover(candidates, "fetch_videos", move |item| {
            let attempts = Arc::clone(&attempts_clone);
            async move {
                let count = attempts.fetch_add(1, Ordering::SeqCst);
                if count == 0 {
                    Err::<String, _>(ExtractorError::Other("加载超时 (20s)".to_string()))
                } else {
                    Ok(format!("success from {}", item))
                }
            }
        })
        .await;

        assert_eq!(result.unwrap(), "success from domain2");
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }
}
