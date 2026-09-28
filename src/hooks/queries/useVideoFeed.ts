import { useCallback, useEffect, useMemo, useRef } from 'react';
import { keepPreviousData, useInfiniteQuery } from '@tanstack/react-query';
import { tauriApi } from '../../api';
import { useCanvasStore } from '../../store/canvasStore';
import { useSiteStore } from '../../store/siteStore';
import { useUIStore } from '../../store/uiStore';
import { VideoInfo, VideoPage } from '../../types';
import { formatExtractorError } from '../../utils/formatError';
import { queryKeys } from './keys';


interface PageResult {
  items: VideoInfo[];
  latestPage: number;
  totalPages: number;
}

/**
 * 每次拉取 2 页，保证卡片池充沛且顺滑
 */
async function fetchTwoPages(
  fetchPage: (page: number) => Promise<VideoPage>,
  startPage: number,
): Promise<PageResult> {
  const page1 = await fetchPage(startPage);
  let items = page1?.items;
  let latestPage = page1?.page || startPage;
  const totalPages = page1?.total_pages || 1;

  const nextP = startPage + 1;
  if (nextP <= totalPages) {
    try {
      const page2 = await fetchPage(nextP);
      if (page2?.items?.length) {
        items = [...items, ...page2.items];
        latestPage = page2.page || nextP;
      }
    } catch {
      // 忽略第2页预取错误
    }
  }
  items = deduplicateCards(items);

  return { items, latestPage, totalPages };
}

export function useVideoFeed() {
  // 各个参数分开订阅，防止无限重渲染
  const currentSite = useSiteStore((s) => s.currentSite);
  const currentUrl = useSiteStore((s) => s.currentUrl);
  const searchKeyword = useSiteStore((s) => s.searchKeyword);
  const currentSort = useSiteStore((s) => s.currentSort);
  const language = useUIStore((s) => s.language);

  // 锁定对象引用： 只要 currentSite、currentUrl、keyword、sort 等原始入参值没有变，queryKey 的内存地址就绝对不变。
  // 触发相机复位： 只有当用户真正点击了不同站点、切换了分类标签、或敲回车搜索导致入参变化时，queryKey 才会产生新引用，从而触发 resetCamera() 将视野归位。
  const queryKey = useMemo(
    () => queryKeys.videoFeed({site: currentSite, url: currentUrl, keyword: searchKeyword, sort: currentSort, lang: language}),
    [currentSite, currentUrl, searchKeyword, currentSort, language]
  )

  useEffect(() => {
    useCanvasStore.getState().resetCamera();
  }, [queryKey]);

  const isQueryEnabled = Boolean(
    currentSite && (searchKeyword || currentUrl)
  );

  const query = useInfiniteQuery<PageResult, unknown>({
    queryKey,
    enabled: isQueryEnabled,
    placeholderData: keepPreviousData,
    queryFn: async ({ pageParam }) => {
      const pageIdx = pageParam as number;
      const fetchParams = {
        page: pageIdx,
        sort_by: currentSort || undefined,
        lang: language,
      };

      // 搜索模式
      if (searchKeyword && currentSite) {
        return fetchTwoPages(
          (p) => tauriApi.searchVideos(currentSite, searchKeyword, { ...fetchParams, page: p }),
          pageIdx
        );
      }

      // 普通浏览模式
      return fetchTwoPages(
        (p) => tauriApi.fetchVideos(currentUrl, { ...fetchParams, page: p }),
        pageIdx
      );
    },
    initialPageParam: 1,
    getNextPageParam: (lastPage) => {
      if (lastPage.latestPage < lastPage.totalPages && lastPage.items.length > 0) {
        return lastPage.latestPage + 1;
      }
      return undefined;
    },
    staleTime: 2 * 60 * 1000, // 2分钟内切换分类/站点可直接复用缓存，无需重新加载
    gcTime: 10 * 60 * 1000,
    refetchOnWindowFocus: false,
  });


  // 错误提示
  useEffect(() => {
    if (query.error) {
      const msg = formatExtractorError(query.error);
      useUIStore.getState().showToast(msg, 'error');
    }
  }, [query.error]);

  // 展平所有分页并去重：冷启动未选站点时展示 fallback cards，一旦选定站点后严格保持站点数据
  const cards = useMemo(() => {
    if (!currentSite) {
      return FALLBACK_CARDS;
    }
    if (!query.data?.pages) return [];
    const all = query.data.pages.flatMap((p) => p.items);
    return deduplicateCards(all);
  }, [currentSite, query.data]);


  // 为了让 fetchNexts 尽可能少变动，避免 InfiniteCanvas 频繁渲染
  // React 官方推荐「用 ref 保存最新值 + 稳定函数」的场景，可以用 useEffectEvent（实验性）或自己封装一个 useEventCallback
  const queryRef = useRef(query);
  queryRef.current = query;
  const fetchNexts = useCallback(() => {
    const q = queryRef.current;
    if (q.hasNextPage && !q.isFetchingNextPage) {
      q.fetchNextPage();
    }
  }, []) 

  return {
    cards,
    fetchNexts
  };
}

function deduplicateCards(cards: VideoInfo[]): VideoInfo[] {
  const seen = new Set<string>();
  return cards.filter((c) => {
    if (seen.has(c.id)) return false;
    seen.add(c.id);
    return true;
  });
}


export const FALLBACK_CARDS: VideoInfo[] = [
  {
    id: "card-01",
    title: "Mountain Sunrise",
    cover_url: "https://picsum.photos/seed/picsum-01/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-02",
    title: "Urban Architecture",
    cover_url: "https://picsum.photos/seed/picsum-02/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-03",
    title: "Forest Trail",
    cover_url: "https://picsum.photos/seed/picsum-03/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-04",
    title: "Ocean Waves",
    cover_url: "https://picsum.photos/seed/picsum-04/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-05",
    title: "Desert Horizon",
    cover_url: "https://picsum.photos/seed/picsum-05/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-06",
    title: "Autumn Leaves",
    cover_url: "https://picsum.photos/seed/picsum-06/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-07",
    title: "Misty Valley",
    cover_url: "https://picsum.photos/seed/picsum-07/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-08",
    title: "Starry Night",
    cover_url: "https://picsum.photos/seed/picsum-08/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-09",
    title: "Coastal Sunset",
    cover_url: "https://picsum.photos/seed/picsum-09/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-10",
    title: "City Lights",
    cover_url: "https://picsum.photos/seed/picsum-10/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-11",
    title: "Snowy Peaks",
    cover_url: "https://picsum.photos/seed/picsum-11/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-12",
    title: "River Bend",
    cover_url: "https://picsum.photos/seed/picsum-12/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-13",
    title: "Green Hills",
    cover_url: "https://picsum.photos/seed/picsum-13/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-14",
    title: "Vintage Street",
    cover_url: "https://picsum.photos/seed/picsum-14/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-15",
    title: "Tropical Shore",
    cover_url: "https://picsum.photos/seed/picsum-15/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-16",
    title: "Deep Canyon",
    cover_url: "https://picsum.photos/seed/picsum-16/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-17",
    title: "Golden Hour Field",
    cover_url: "https://picsum.photos/seed/picsum-17/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-18",
    title: "Abstract Shadows",
    cover_url: "https://picsum.photos/seed/picsum-18/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-19",
    title: "Lakeside Reflection",
    cover_url: "https://picsum.photos/seed/picsum-19/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-20",
    title: "Pine Canopy",
    cover_url: "https://picsum.photos/seed/picsum-20/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-21",
    title: "Rocky Shoreline",
    cover_url: "https://picsum.photos/seed/picsum-21/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-22",
    title: "Morning Fog",
    cover_url: "https://picsum.photos/seed/picsum-22/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-23",
    title: "Sunset Over Bridge",
    cover_url: "https://picsum.photos/seed/picsum-23/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-24",
    title: "Night Sky Panorama",
    cover_url: "https://picsum.photos/seed/picsum-24/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-25",
    title: "Alpine Meadow",
    cover_url: "https://picsum.photos/seed/picsum-25/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-26",
    title: "Neon Alley",
    cover_url: "https://picsum.photos/seed/picsum-26/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-27",
    title: "Cascading Waterfall",
    cover_url: "https://picsum.photos/seed/picsum-27/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-28",
    title: "Bamboo Grove",
    cover_url: "https://picsum.photos/seed/picsum-28/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-29",
    title: "Desert Dunes",
    cover_url: "https://picsum.photos/seed/picsum-29/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-30",
    title: "Glacier Lagoon",
    cover_url: "https://picsum.photos/seed/picsum-30/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-31",
    title: "Cherry Blossom Path",
    cover_url: "https://picsum.photos/seed/picsum-31/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-32",
    title: "Quiet Harbor",
    cover_url: "https://picsum.photos/seed/picsum-32/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-33",
    title: "Volcanic Ridge",
    cover_url: "https://picsum.photos/seed/picsum-33/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-34",
    title: "Prairie Sunrise",
    cover_url: "https://picsum.photos/seed/picsum-34/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-35",
    title: "Nordic Fjord",
    cover_url: "https://picsum.photos/seed/picsum-35/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-36",
    title: "Old Town Square",
    cover_url: "https://picsum.photos/seed/picsum-36/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-37",
    title: "Coastal Cliffs",
    cover_url: "https://picsum.photos/seed/picsum-37/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-38",
    title: "Emerald Forest",
    cover_url: "https://picsum.photos/seed/picsum-38/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-39",
    title: "Lavender Fields",
    cover_url: "https://picsum.photos/seed/picsum-39/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-40",
    title: "Starlit Dunes",
    cover_url: "https://picsum.photos/seed/picsum-40/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-41",
    title: "Subway Motion",
    cover_url: "https://picsum.photos/seed/picsum-41/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-42",
    title: "Crystal Cavern",
    cover_url: "https://picsum.photos/seed/picsum-42/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-43",
    title: "Highland Creek",
    cover_url: "https://picsum.photos/seed/picsum-43/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-44",
    title: "Foggy Pier",
    cover_url: "https://picsum.photos/seed/picsum-44/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-45",
    title: "Sunflower Valley",
    cover_url: "https://picsum.photos/seed/picsum-45/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-46",
    title: "Rainy Boulevard",
    cover_url: "https://picsum.photos/seed/picsum-46/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-47",
    title: "Winter Solitude",
    cover_url: "https://picsum.photos/seed/picsum-47/640/360",
    detail_page_url: "https://picsum.photos"
  },
  {
    id: "card-48",
    title: "Aurora Borealis",
    cover_url: "https://picsum.photos/seed/picsum-48/640/360",
    detail_page_url: "https://picsum.photos"
  }
];