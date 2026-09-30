import { create } from 'zustand';
import i18n from '../i18n';
import { Language, MediaDetail, VideoInfo } from '../types';

export type ActiveDrawer = 'none' | 'siteNav' | 'downloads' | 'settings';

export type ToastType = 'error' | 'warning' | 'info' | 'success';

export interface ToastItem {
  id: string;
  type: ToastType;
  message: string;
}

const COLLAPSED_TAGS_STORAGE_KEY = 'avdl_collapsed_tag_groups';
const loadCollapsedTagGroups = (): Record<string, boolean> => {
  try {
    const raw = localStorage.getItem(COLLAPSED_TAGS_STORAGE_KEY);
    return raw ? JSON.parse(raw) : {};
  } catch {
    return {};
  }
};

export interface CardAnchorRect {
  left: number;
  top: number;
  width: number;
  height: number;
  bottom: number;
  right: number;
}

export interface UIState {
  language: Language;
  activeDrawer: ActiveDrawer;
  collapsedTagGroups: Record<string, boolean>;
  toasts: ToastItem[];
  selectedCard: VideoInfo | null;
  selectedCardRect: CardAnchorRect | null;
  playingMedia: MediaDetail | null;
  playingCard: VideoInfo | null;
  isVideoLoading: boolean;

  // Video feed loading action
  setVideoLoading: (loading: boolean) => void;

  // Language action
  setLanguage: (language: Language) => void;

  // Drawer actions
  toggleDrawer: (drawer: Exclude<ActiveDrawer, 'none'>) => void;
  closeDrawer: () => void;
  toggleTagGroupCollapsed: (groupKey: string) => void;

  // Modal actions
  openDetail: (card: VideoInfo, rect?: CardAnchorRect | null) => void;
  closeDetail: () => void;
  playMedia: (detail: MediaDetail, card?: VideoInfo | null) => void;
  playCard: (card: VideoInfo) => void;
  closePlayer: () => void;

  // Toast actions
  showToast: (message: string, type?: ToastType, duration?: number) => void;
  dismissToast: (id: string) => void;
}

export const useUIStore = create<UIState>((set, get) => ({
  language: (i18n.language as Language) || 'zh-CN',
  activeDrawer: 'none',
  collapsedTagGroups: loadCollapsedTagGroups(),
  toasts: [],
  selectedCard: null,
  selectedCardRect: null,
  playingMedia: null,
  playingCard: null,
  isVideoLoading: false,

  setVideoLoading: (isVideoLoading: boolean) => set({ isVideoLoading }),

  setLanguage: (language: Language) => {
    i18n.changeLanguage(language);
    set({ language });
  },

  toggleDrawer: (drawer) => {
    const current = get().activeDrawer;
    set({ activeDrawer: current === drawer ? 'none' : drawer });
  },

  closeDrawer: () => {
    set({ activeDrawer: 'none' });
  },

  toggleTagGroupCollapsed: (groupKey: string) => {
    set((state) => {
      const updated = {
        ...state.collapsedTagGroups,
        [groupKey]: !state.collapsedTagGroups[groupKey],
      };
      try {
        localStorage.setItem(COLLAPSED_TAGS_STORAGE_KEY, JSON.stringify(updated));
      } catch {}
      return { collapsedTagGroups: updated };
    });
  },

  openDetail: (card: VideoInfo, rect?: CardAnchorRect | null) => {
    set({ selectedCard: card, selectedCardRect: rect ?? null });
  },

  closeDetail: () => {
    set({ selectedCard: null, selectedCardRect: null });
  },

  playMedia: (detail: MediaDetail, card?: VideoInfo | null) => {
    set({ playingMedia: detail, playingCard: card ?? null, selectedCard: null, selectedCardRect: null });
  },

  playCard: (card: VideoInfo) => {
    set({ playingCard: card, playingMedia: null, selectedCard: null, selectedCardRect: null });
  },

  closePlayer: () => {
    set({ playingMedia: null, playingCard: null });
  },

  showToast: (message: string, type: ToastType = 'info', duration: number = 4000) => {
    const id = `toast-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;
    set((state) => ({
      // 最多同时展示 4 条 toast，多余的淘汰最旧的
      toasts: [...state.toasts.slice(-3), { id, type, message }],
    }));

    if (duration > 0) {
      setTimeout(() => {
        get().dismissToast(id);
      }, duration);
    }
  },

  dismissToast: (id: string) => {
    set((state) => ({
      toasts: state.toasts.filter((t) => t.id !== id),
    }));
  },
}));
