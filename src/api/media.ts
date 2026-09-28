import { invoke } from '@tauri-apps/api/core';
import {
  FetchParams,
  MediaDetail,
  ProviderSite,
  SiteManifest,
  VideoInfo,
  VideoPage,
} from '../types';

function composeFetchParams(params?: FetchParams): FetchParams {
  return {
    lang: 'zh-TW',
    ...params,
  };
}

export const mediaService = {
  /**
   * 拉取指定 URL 的视频分页列表
   */
  async fetchVideos(url: string, params?: FetchParams): Promise<VideoPage> {
    return invoke<VideoPage>('list_videos', { url, params: composeFetchParams(params) });
  },

  /**
   * 获取所有站点的元信息声明
   */
  async getSiteManifests(): Promise<SiteManifest[]> {
    return invoke<SiteManifest[]>('get_site_manifests');
  },

  /**
   * 搜索视频
   */
  async searchVideos(
    site: ProviderSite,
    keyword: string,
    params?: FetchParams
  ): Promise<VideoPage> {
    return invoke<VideoPage>('search_videos', { site, keyword, params: composeFetchParams(params) });
  },

  /**
   * 提取单片详细信息与正片播放源
   */
  async extractMedia(card: VideoInfo, params?: FetchParams): Promise<MediaDetail> {
    return invoke<MediaDetail>('extract_media', { url: card.detail_page_url, params: composeFetchParams(params) });
  },

  /**
   * 构建卡片的本地预览播放地址 (基于 stream:// 协议)
   */
  buildPreviewStreamUrl(card: VideoInfo): string | null {
    if (!card.preview_url) return null;
    return `stream://localhost/proxy?url=${encodeURIComponent(
      card.preview_url
    )}${card.referer ? `&referer=${encodeURIComponent(card.referer)}` : ''}${
      card.ua ? `&ua=${encodeURIComponent(card.ua)}` : ''
    }&is_preview=1`;
  },

  /**
   * 构建通过后端代理的流式播放地址 (用于正片视频或长视频流)
   */
  buildStreamUrl(url: string, referer?: string, ua?: string): string {
    if (!url) return '';
    if (url.startsWith('stream://')) return url;
    let proxyUrl = `stream://localhost/proxy?url=${encodeURIComponent(url)}`;
    if (referer) proxyUrl += `&referer=${encodeURIComponent(referer)}`;
    if (ua) proxyUrl += `&ua=${encodeURIComponent(ua)}`;
    return proxyUrl;
  },
};
