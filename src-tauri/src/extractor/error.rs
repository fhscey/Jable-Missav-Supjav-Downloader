use crate::config::Language;
use crate::extractor::model::ProviderSite;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ExtractorError {
    #[error("Unsupported URL: {0}")]
    UnsupportedUrl(String),

    #[error("Unsupported language: {0}")]
    UnsupportedLanguage(Language),

    #[error("Unsupported site: {0}")]
    UnsupportedSite(ProviderSite),

    #[error("HTTP request failed: {0}")]
    Http(String),

    #[error("Get HTML from {0} failed: {1}")]
    HtmlGetFailed(String, String),

    #[error("Get {0} from {1} failed")]
    HtmlParse(String, String),

    #[error("Media stream source not found")]
    MediaNotFound,

    #[error("Cloudflare challenge required for domain: {domain}, url: {target_url}")]
    CloudflareBlocked { domain: String, target_url: String },

    #[error("Other error: {0}")]
    Other(String),
}

impl ExtractorError {
    /// 判断该错误是否应当触发多域名/候选站点故障切换重试 (Failover)
    /// 只有类似网络超时、连接失败等暂时性网络通道故障才切换候选重试；
    /// 像参数错误（如不支持的语言、非法 URL 等）及解析错误应直接返回并反馈到前端。
    pub fn is_failover_eligible(&self) -> bool {
        match self {
            // 参数错误 / 站点不支持 / 解析格式错误：镜像域名无法解决，立即返回上层
            ExtractorError::UnsupportedLanguage(_)
            | ExtractorError::UnsupportedUrl(_)
            | ExtractorError::UnsupportedSite(_)
            | ExtractorError::HtmlParse(_, _) => false,

            // 明确的网络故障 / 获取失败 / 域名拦截：可切换候选域名重试
            ExtractorError::Http(_)
            | ExtractorError::HtmlGetFailed(_, _)
            | ExtractorError::CloudflareBlocked { .. } => true,

            // 媒体流未找到：按规则属于内容本身缺失，不进行切换重试
            ExtractorError::MediaNotFound => false,

            // Other 类型：检查是否包含网络/超时相关特征
            ExtractorError::Other(msg) => {
                let lower = msg.to_lowercase();
                lower.contains("timeout")
                    || lower.contains("超时")
                    || lower.contains("timed out")
                    || lower.contains("connection refused")
                    || lower.contains("connection reset")
                    || lower.contains("connect")
                    || lower.contains("network")
                    || lower.contains("econnreset")
                    || lower.contains("502")
                    || lower.contains("503")
                    || lower.contains("504")
            }
        }
    }
}
