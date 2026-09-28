import { useQuery } from '@tanstack/react-query';
import { tauriApi } from '../../api';
import { useSiteStore } from '../../store/siteStore';
import { SiteManifest, SortRule } from '../../types';
import { queryKeys } from './keys';

export const FALLBACK_MANIFEST: SiteManifest = {
  site: 'jable',
  name: 'Jable TV',
  primary_domain: 'jable.tv',
  available_domains: ['jable.tv'],
  default_url: 'https://jable.tv/latest-updates/',
  quick_links: [],
  categories: [],
  tags: [],
};

/**
 * 站点清单（SiteManifests）获取 Hook
 *
 * 缓存策略设计：
 * - staleTime (Infinity)：支持的站点与分类元数据在整个运行周期几乎是静态的，只拉取一次，全局多组件共享
 */
export function useSiteManifests() {
  return useQuery<SiteManifest[], unknown>({
    queryKey: queryKeys.manifests(),
    queryFn: () => tauriApi.getSiteManifests(),
    staleTime: Infinity,
    gcTime: Infinity,
    refetchOnWindowFocus: false,
  });
}

/**
 * 派生 Hook：根据当前选中的站点 currentSite 动态匹配对应 Manifest（未选中时返回 null）
 */
export function useActiveManifest(): SiteManifest | null {
  const { data: manifests } = useSiteManifests();
  const currentSite = useSiteStore((s) => s.currentSite);

  if (!currentSite || !manifests || manifests.length === 0) {
    return null;
  }

  return manifests.find((m) => m.site === currentSite) || null;
}

/**
 * 根据当前 URL 动态解析命中的排序规则
 */
export function resolveSortRule(manifest?: SiteManifest | null, currentUrl?: string): SortRule | null {
  if (!manifest?.sort_rules || !currentUrl) return null;

  for (const rule of manifest.sort_rules) {
    if (rule.match_patterns.includes('*')) return rule;
    if (rule.match_patterns.some((p) => currentUrl.includes(p))) return rule;
  }

  return null;
}

/**
 * 派生 Hook：获取当前 URL 命中的排序规则
 */
export function useCurrentSortRule() {
  const activeManifest = useActiveManifest();
  const currentUrl = useSiteStore((s) => s.currentUrl);
  return resolveSortRule(activeManifest, currentUrl);
}
