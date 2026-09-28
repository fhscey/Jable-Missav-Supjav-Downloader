import { Language, ProviderSite } from '../../types';

export interface VideoFeedKeyParams {
  site: ProviderSite | null;
  url?: string;
  keyword?: string;
  sort?: string;
  lang: Language;
}

/**
 * 集中管理所有 Query Keys，提供类型安全的 Key 工厂
 */
export const queryKeys = {
  manifests: () => ['siteManifests'] as const,
  config: () => ['appConfig'] as const,
  mediaDetail: (cardId: string, lang: Language) => ['mediaDetail', cardId, lang] as const,
  videoFeed: (params: VideoFeedKeyParams) => ['videoFeed', params] as const,
};

