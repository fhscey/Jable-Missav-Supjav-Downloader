use tauri::State;

use crate::extractor::{
    ExtractorError, FetchParams, MediaDetail, ProviderSite, SiteManifest, VideoInfo, VideoPage,
};
use crate::network::stream;
use crate::AppState;

/// 提取指定 URL 的媒体播放流与详情（支持自动多域名容灾重试）
#[tauri::command]
pub async fn extract_media(
    url: String,
    params: Option<FetchParams>,
    state: State<'_, AppState>,
) -> Result<MediaDetail, ExtractorError> {
    state.extractor.extract(&url, params.as_ref()).await
}

/// 获取所有站点的元信息声明（包含可用域名、快捷入口、全部分类与分组标签）
#[tauri::command]
pub async fn get_site_manifests(
    state: State<'_, AppState>,
) -> Result<Vec<SiteManifest>, ExtractorError> {
    Ok(state.extractor.get_all_site_manifests())
}

/// 拉取指定 URL 的视频分页列表（首页、分类页、标签页等，支持自动多域名容灾重试）
#[tauri::command]
pub async fn list_videos(
    url: String,
    params: Option<FetchParams>,
    state: State<'_, AppState>,
) -> Result<VideoPage, ExtractorError> {
    state.extractor.list_videos(&url, params.as_ref()).await
}

/// 站点内容关键词检索（支持抓取参数）
#[tauri::command]
pub async fn search_videos(
    site: ProviderSite,
    keyword: String,
    params: Option<FetchParams>,
    state: State<'_, AppState>,
) -> Result<VideoPage, ExtractorError> {
    state
        .extractor
        .search(site, &keyword, params.as_ref())
        .await
}

/// 获取卡片的本地流式预览播放地址（通过 stream:// 本地代理协议）
#[tauri::command]
pub fn get_preview_stream_url(card: VideoInfo) -> Option<String> {
    let preview = card.preview_url.as_deref().filter(|s| !s.is_empty())?;
    Some(stream::build_stream_url(
        preview,
        card.referer.as_deref(),
        card.ua.as_deref(),
        true,
    ))
}
