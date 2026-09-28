use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::config::Language;

/// 站点枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderSite {
    Jable,
    Missav,
    Supjav,
}

impl std::fmt::Display for ProviderSite {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderSite::Jable => write!(f, "jable"),
            ProviderSite::Missav => write!(f, "missav"),
            ProviderSite::Supjav => write!(f, "supjav"),
        }
    }
}

impl std::str::FromStr for ProviderSite {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "jable" => Ok(ProviderSite::Jable),
            "missav" => Ok(ProviderSite::Missav),
            "supjav" => Ok(ProviderSite::Supjav),
            _ => Err(()),
        }
    }
}

/// 列表页/检索结果的视频卡片（轻量摘要）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoInfo {
    pub id: String,              // 同 MediaDetail.id
    pub title: String,           // 标题
    pub cover_url: String,       // 封面缩略图
    pub detail_page_url: String, // 详情页链接
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<String>, // 时长
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview_url: Option<String>, // 预览视频链接
    #[serde(skip_serializing_if = "Option::is_none")]
    pub referer: Option<String>, // 列表页来源引用 (用于下载预览等)
    #[serde(alias = "user_agent", skip_serializing_if = "Option::is_none")]
    pub ua: Option<String>, // User Agent
}

/// 详情页核心解析产物
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaDetail {
    pub id: String,              // 唯一，用详情页链接代替
    pub title: String,           // 标题
    pub cover_url: String,       // 封面链接
    pub detail_page_url: String, // 详情页链接
    pub stream_url: String,      // 视频流链接, m3u8 或 mp4等
    #[serde(skip_serializing_if = "Option::is_none")]
    pub referer: Option<String>, // 用于请求视频流
    pub categories: Vec<NavItem>, // 分类搜索链接
    pub tags: Vec<NavItem>,      // 标签搜索链接
    pub actresses: Vec<NavItem>, // 女优搜索链接
}

impl std::fmt::Display for MediaDetail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut lines = Vec::new();
        lines.push("MediaDetail:".to_string());
        lines.push(format!("  ID: {}", self.id));
        lines.push(format!("  Title: {}", self.title));
        lines.push(format!("  Detail URL: {}", self.detail_page_url));
        lines.push(format!("  Stream URL: {}", self.stream_url));
        if !self.cover_url.is_empty() {
            lines.push(format!("  Cover: {}", self.cover_url));
        }
        if let Some(ref r) = self.referer {
            lines.push(format!("  Referer: {}", r));
        }
        if !self.actresses.is_empty() {
            let names: Vec<&str> = self.actresses.iter().map(|n| n.name.as_str()).collect();
            lines.push(format!("  Actresses: [{}]", names.join(", ")));
        }
        if !self.categories.is_empty() {
            let names: Vec<&str> = self.categories.iter().map(|n| n.name.as_str()).collect();
            lines.push(format!("  Categories: [{}]", names.join(", ")));
        }
        if !self.tags.is_empty() {
            let names: Vec<&str> = self.tags.iter().map(|n| n.name.as_str()).collect();
            lines.push(format!("  Tags: [{}]", names.join(", ")));
        }
        write!(f, "{}", lines.join("\n"))
    }
}

/// 统一分页结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoPage {
    pub items: Vec<VideoInfo>,
    pub page: usize,
    pub total_pages: usize,
}

/// 抓取选项与参数（支持分页、语言与排序）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FetchParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<Language>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_by: Option<String>,
}

/// 基础导航项（统一快捷链接、分类与单标签）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavItem {
    pub name: String, // key name 英文，前端国际化用
    pub url: String,
}

/// 排序选项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SortOption {
    pub name: String,  // 标识/英文 key，前端国际化或展示用
    pub value: String, // 实际传给 sort_by 的参数值
}

/// 某类 Path 场景下的排序规则组
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SortRule {
    pub match_patterns: Vec<String>, // 匹配 URL 路径特征片段，如 ["categories", "tags"] 或 ["*"]
    pub options: Vec<SortOption>,    // 可选项列表（空数组表示该场景显式不支持排序）
}

/// 侧边栏/标签项分组（保持定义序，避免无序映射）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagGroup {
    pub group: String,
    pub items: Vec<NavItem>,
}

/// 站点元信息声明（包含可用域名、快捷入口、分类与标签）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteManifest {
    pub site: ProviderSite,
    pub name: String,
    pub primary_domain: String,
    pub available_domains: Vec<String>,
    pub default_url: String,
    pub supported_languages: HashMap<Language, String>, // 如果不支持简体中文，全部退化成繁体中文
    pub quick_links: Vec<NavItem>,
    pub categories: Vec<NavItem>,
    pub tags: Vec<TagGroup>, // 如果没有 group，就直接变成 category
    #[serde(default)]
    pub sort_rules: Vec<SortRule>,
}
