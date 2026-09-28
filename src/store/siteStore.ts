import { create } from 'zustand';
import { NavItem, ProviderSite } from '../types';

export interface SiteFilterState {
  currentSite: ProviderSite | null;
  searchKeyword: string;
  currentSort: string;
  currentUrl: string;

  // 纯 UI Setters / Actions
  setCurrentSite: (currentSite: ProviderSite | null) => void;
  setCurrentUrl: (currentUrl: string) => void;

  handleSearchSubmit: (keyword: string, sortValue?: string) => void;
  exitSearch: () => void;
  handleSortSelect: (sortValue: string) => void;
  handleSelectSite: (siteKey: ProviderSite, initialUrl?: string) => void;
  handleSelectNavItem: (item: NavItem) => void;
  disconnectSite: () => void;
}

export const useSiteStore = create<SiteFilterState>((set, get) => ({
  currentSite: null,
  searchKeyword: '',
  currentSort: '',
  currentUrl: '',

  setCurrentSite: (currentSite) =>
    set({
      currentSite,
      currentUrl: '',
      searchKeyword: '',
      currentSort: '',
    }),
  setCurrentUrl: (currentUrl: string) => set({ currentUrl }),

  /**
   * 提交搜索（设置全局生效的搜索关键词）
   */
  handleSearchSubmit: (keyword: string, sortValue?: string) => {
    const kw = keyword.trim();
    const activeSort = sortValue !== undefined ? sortValue : get().currentSort;

    if (!kw) {
      set({ searchKeyword: '', currentSort: activeSort });
      return;
    }

    set({ searchKeyword: kw, currentSort: activeSort });
  },

  /**
   * 退出搜索
   */
  exitSearch: () => {
    set({ searchKeyword: '' });
  },

  /**
   * 切换排序
   */
  handleSortSelect: (sortValue: string) => {
    set({ currentSort: sortValue });
  },

  /**
   * 切换站点
   */
  handleSelectSite: (siteKey: ProviderSite, initialUrl: string = '') => {
    set({
      currentSite: siteKey,
      currentUrl: initialUrl,
      searchKeyword: '',
      currentSort: '',
    });
  },

  /**
   * 分类 / 标签导航点击
   */
  handleSelectNavItem: (item: NavItem) => {
    let targetSort = '';
    try {
      const parsed = new URL(item.url);
      targetSort = parsed.searchParams.get('sort_by') || parsed.searchParams.get('sort') || '';
    } catch {}
    set({
      currentUrl: item.url,
      currentSort: targetSort,
      searchKeyword: '',
    });
  },

  /**
   * 一键断开站点连接与重置检索/分类状态
   */
  disconnectSite: () => {
    set({
      currentSite: null,
      currentUrl: '',
      searchKeyword: '',
      currentSort: '',
    });
  },
}));
