pub mod jable;
pub mod missav;
pub mod supjav;

pub use jable::JableProvider;
pub use missav::MissavProvider;
pub use supjav::SupjavProvider;

use async_trait::async_trait;

use crate::extractor::{
    ExtractorError, FetchParams, MediaDetail, ProviderSite, SiteManifest, VideoPage,
};
use crate::network::{HttpClient, WebviewEngine};

#[async_trait]
pub trait SiteProvider: Send + Sync {
    /// 对应的站点
    fn site(&self) -> ProviderSite;

    /// 获取当前站点的全部候选可用域名配置（首项为主域名）
    fn domains(&self) -> &[String];

    /// 获取当前站点的主域名
    fn primary_domain(&self) -> &str {
        self.domains().first().map(|s| s.as_str()).unwrap_or("")
    }

    /// 获取当前站点的清单元数据（直接返回引用，零克隆）
    fn manifest(&self) -> &SiteManifest;

    /// 是否能处理该 URL
    fn can_handle(&self, url: &str) -> bool;

    /// 提取视频详情及可用播放流（支持语言等选项）
    async fn extract(
        &self,
        webview: &WebviewEngine,
        client: &HttpClient,
        url: &str,
        params: Option<&FetchParams>,
    ) -> Result<MediaDetail, ExtractorError>;

    /// 获取分类或通用列表页（支持分页、排序与语言等参数）
    async fn list(
        &self,
        webview: &WebviewEngine,
        url: &str,
        params: Option<&FetchParams>,
    ) -> Result<VideoPage, ExtractorError>;

    /// 站点关键字检索 URL 构造（若站点不支持检索可返回 None）
    fn search_url(&self, keyword: &str, params: Option<&FetchParams>) -> Option<String>;
}
