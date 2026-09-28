use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::extractor::ProviderSite;
use crate::network::http_client::ProxyMode;

fn default_auto_check_update() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// 配置文件自身所在的绝对路径（方便前端展示或一键打开）
    pub config_path: PathBuf,

    /// 默认下载保存路径
    pub download_dir: PathBuf,

    /// 全局最大并发下载任务数
    pub max_concurrent_tasks: usize,

    /// 全局限速 (bytes/s)，0 为不限速
    pub max_download_speed: u64,

    /// 偏好影片画质 (如 PreferredQuality::Best, PreferredQuality::P1080)
    #[serde(default)]
    pub preferred_quality: PreferredQuality,

    /// 界面语言
    pub language: Language,

    /// 是否开启日志记录
    pub enable_logging: bool,

    /// 网络与代理设置
    pub proxy_mode: ProxyMode,

    /// 是否自动检查更新
    #[serde(default = "default_auto_check_update")]
    pub auto_check_update: bool,

    /// 删除任务时是否同时删除本地文件
    #[serde(default)]
    pub delete_file_on_remove: bool,

    /// 站点可用域名表（首项为主域名，其余为故障切换重试域名）
    #[serde(default = "default_site_domains")]
    pub site_domains: HashMap<ProviderSite, Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    #[serde(rename = "en")]
    EN,
    #[serde(rename = "ja")]
    JA,
    #[serde(rename = "zh-CN")]
    ZHCN,
    #[serde(rename = "zh-TW")]
    ZHTW,
}
impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Language::EN => write!(f, "en"),
            Language::JA => write!(f, "ja"),
            Language::ZHCN => write!(f, "zh-CN"),
            Language::ZHTW => write!(f, "zh-TW"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum PreferredQuality {
    #[default]
    #[serde(rename = "highest", alias = "best", alias = "auto")]
    Highest,
    #[serde(rename = "1080p")]
    P1080,
    #[serde(rename = "720p")]
    P720,
    #[serde(rename = "480p")]
    P480,
    #[serde(rename = "lowest")]
    Lowest,
}

impl PreferredQuality {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Highest => "highest",
            Self::P1080 => "1080p",
            Self::P720 => "720p",
            Self::P480 => "480p",
            Self::Lowest => "lowest",
        }
    }

    pub fn target_height(&self) -> Option<u64> {
        match self {
            Self::Highest | Self::Lowest => None,
            Self::P1080 => Some(1080),
            Self::P720 => Some(720),
            Self::P480 => Some(480),
        }
    }
}

impl std::str::FromStr for PreferredQuality {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "highest" | "best" | "auto" => Ok(Self::Highest),
            "1080p" | "fhd" => Ok(Self::P1080),
            "720p" | "hd" => Ok(Self::P720),
            "480p" | "sd" => Ok(Self::P480),
            "lowest" => Ok(Self::Lowest),
            _ => Err(format!("Unknown preferred quality: {}", s)),
        }
    }
}

impl std::fmt::Display for PreferredQuality {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

pub fn default_site_domains() -> HashMap<ProviderSite, Vec<String>> {
    let mut map = HashMap::new();
    map.insert(
        ProviderSite::Jable,
        crate::extractor::providers::JableProvider::default_domains(),
    );
    map.insert(
        ProviderSite::Missav,
        crate::extractor::providers::MissavProvider::default_domains(),
    );
    map.insert(
        ProviderSite::Supjav,
        crate::extractor::providers::SupjavProvider::default_domains(),
    );
    map
}

impl Default for AppConfig {
    fn default() -> Self {
        let download_dir = dirs::download_dir().unwrap_or_else(|| PathBuf::from("./Downloads"));
        Self {
            config_path: PathBuf::new(),
            download_dir,
            max_concurrent_tasks: 3,
            max_download_speed: 0,
            preferred_quality: PreferredQuality::Highest,
            language: Language::ZHTW,
            enable_logging: true,
            proxy_mode: ProxyMode::System,
            auto_check_update: true,
            delete_file_on_remove: false,
            site_domains: default_site_domains(),
        }
    }
}

const DEFAULT_CONFIG_PATH: &str = "config.json";

impl AppConfig {
    /// 在指定目录下读取或初始化 config.json
    pub fn load_or_init(app_data_dir: &PathBuf) -> Result<Self, String> {
        if !app_data_dir.exists() {
            fs::create_dir_all(app_data_dir)
                .map_err(|e| format!("Failed to create app data directory: {}", e))?;
        }

        let path = app_data_dir.join(DEFAULT_CONFIG_PATH);

        let config = if path.exists() {
            let bytes = fs::read(&path).map_err(|e| format!("Failed to read config: {}", e))?;
            serde_json::from_slice::<AppConfig>(&bytes)
                .map_err(|e| format!("Failed to deserialize config: {}", e))?
        } else {
            let cfg = AppConfig {
                config_path: path.clone(),
                ..Default::default()
            };
            let bytes = serde_json::to_vec_pretty(&cfg)
                .map_err(|e| format!("Failed to serialize default config: {}", e))?;
            fs::write(&path, bytes)
                .map_err(|e| format!("Failed to write default config: {}", e))?;
            cfg
        };

        Ok(config)
    }

    /// 保存配置到自身 config_path
    pub fn save(&self) -> Result<(), String> {
        if let Some(parent) = self.config_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create config parent directory: {}", e))?;
            }
        }
        let bytes =
            serde_json::to_vec_pretty(self).map_err(|e| format!("Serialization error: {}", e))?;
        fs::write(&self.config_path, bytes).map_err(|e| format!("Failed to write config: {}", e))
    }
}
