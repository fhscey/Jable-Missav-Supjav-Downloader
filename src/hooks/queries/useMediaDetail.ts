import { useQuery } from '@tanstack/react-query';
import { tauriApi } from '../../api';
import { Language, MediaDetail, VideoInfo } from '../../types';
import { formatExtractorError } from '../../utils/formatError';
import { queryKeys } from './keys';

export interface UseMediaDetailOptions {
  enabled?: boolean;
  initialData?: MediaDetail | null;
}

export interface UseMediaDetailResult {
  detail: MediaDetail | null;
  loading: boolean;
  isLoading: boolean;
  isFetching: boolean;
  error: string;
  refetch: () => void;
}

/**
 * 视频详情获取 Hook
 *
 * 缓存策略设计：
 * - staleTime (30分钟)：详情内容相对静态，30 分钟内同一视频直接秒开
 * - gcTime (60分钟)：保留未激活缓存 1 小时
 * - 支持 initialData 传入（如从上层已持有 detail 的组件直传），无需重复请求
 */
export function useMediaDetail(
  card?: VideoInfo | null,
  lang: Language = 'zh-CN',
  options?: UseMediaDetailOptions
): UseMediaDetailResult {
  const cardKey = card ? card.id || card.detail_page_url : '';
  const hasStreamUrl = Boolean(options?.initialData?.stream_url);

  // 如果已经具备有效 stream_url 的 initialData，则不需要再发起请求，除非显式 refetch
  const isEnabled = Boolean(cardKey) && (options?.enabled ?? true) && !hasStreamUrl;

  const { data, isPending, isFetching, error, refetch } = useQuery<MediaDetail, unknown>({
    queryKey: queryKeys.mediaDetail(cardKey, lang),
    queryFn: () => tauriApi.extractMedia(card!, { lang }),
    enabled: isEnabled,
    initialData: options?.initialData ?? undefined,
    staleTime: 30 * 60 * 1000,
    gcTime: 60 * 60 * 1000,
    refetchOnWindowFocus: false,
    retry: 1,
  });

  // 严格准确的加载状态：启用请求且正在获取（包括首次 pending 或后续 refetch）
  const isLoading = Boolean(isEnabled && (isPending || isFetching));

  return {
    detail: data ?? null,
    loading: isLoading,
    isLoading,
    isFetching,
    error: error ? formatExtractorError(error) : '',
    refetch,
  };
}
