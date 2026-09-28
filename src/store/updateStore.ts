import { create } from 'zustand';
import { UpdateInfo } from '../types';
import { tauriApi } from '../api';

const SKIPPED_VERSION_KEY = 'avdl_skipped_update_version';

const getInitialSkippedVersion = (): string | null => {
  try {
    return localStorage.getItem(SKIPPED_VERSION_KEY);
  } catch {
    return null;
  }
};

export interface UpdateState {
  updateInfo: UpdateInfo | null;
  hasUpdate: boolean;
  showDialog: boolean;
  skippedVersion: string | null;

  // Actions
  initAutoCheck: () => Promise<void>;
  setUpdateAvailable: (info: UpdateInfo) => void;
  openDialog: () => void;
  closeDialog: () => void;
  skipVersion: (version: string) => void;
  remindLater: () => void;
  updateNow: () => Promise<void>;
}

export const useUpdateStore = create<UpdateState>((set, get) => ({
  updateInfo: null,
  hasUpdate: false,
  showDialog: false,
  skippedVersion: getInitialSkippedVersion(),

  initAutoCheck: async () => {
    try {
      const config = await tauriApi.getConfig();
      if (!config?.auto_check_update) return;

      const info = await tauriApi.checkForUpdate();
      if (!info.updateAvailable) return;

      const skipped = get().skippedVersion;
      if (skipped && skipped === info.latestVersion) {
        // 用户已选择跳过此版本：不红点、不弹窗
        set({ updateInfo: info, hasUpdate: false, showDialog: false });
        return;
      }

      // 发现新版本且未被跳过：静默点亮红点，不霸屏强弹 Dialog
      set({
        updateInfo: info,
        hasUpdate: true,
        showDialog: false,
      });
    } catch (err) {
      console.warn('[UpdateStore] Silent auto check failed:', err);
    }
  },

  setUpdateAvailable: (info: UpdateInfo) => {
    const skipped = get().skippedVersion;
    const isSkipped = Boolean(skipped && skipped === info.latestVersion);
    set({
      updateInfo: info,
      hasUpdate: !isSkipped,
      showDialog: true,
    });
  },

  openDialog: () => {
    if (get().updateInfo) {
      set({ showDialog: true });
    }
  },

  closeDialog: () => {
    set({ showDialog: false });
  },

  skipVersion: (version: string) => {
    try {
      localStorage.setItem(SKIPPED_VERSION_KEY, version);
    } catch {}
    set({
      skippedVersion: version,
      hasUpdate: false,
      showDialog: false,
    });
  },

  remindLater: () => {
    // 稍后提醒：保留小红点，仅关闭当前弹窗
    set({ showDialog: false });
  },

  updateNow: async () => {
    const url = get().updateInfo?.releaseUrl;
    if (url) {
      await tauriApi.openUrl(url);
    }
    // 打开浏览器后关闭弹窗，保留小红点
    set({ showDialog: false });
  },
}));
