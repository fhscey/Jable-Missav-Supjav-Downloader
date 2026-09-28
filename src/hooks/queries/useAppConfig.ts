import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { tauriApi } from '../../api';
import i18n from '../../i18n';
import { useUIStore } from '../../store/uiStore';
import { AppConfig, DeleteFileMode } from '../../types';
import { queryKeys } from './keys';

// 本地持久化储存删除模式（纯客户端交互偏好）
let globalDeleteFileMode: DeleteFileMode = 'Ask';

export function getDeleteFileMode(): DeleteFileMode {
  return globalDeleteFileMode;
}

export function setDeleteFileMode(mode: DeleteFileMode) {
  globalDeleteFileMode = mode;
}

/**
 * 获取应用配置 Hook
 */
export function useAppConfig() {
  return useQuery<AppConfig, unknown>({
    queryKey: queryKeys.config(),
    queryFn: async () => {
      const config = await tauriApi.getConfig();
      if (config?.language && i18n.language !== config.language) {
        useUIStore.getState().setLanguage(config.language);
      }
      return config;
    },
    staleTime: 5 * 60 * 1000,
    gcTime: Infinity,
    refetchOnWindowFocus: false,
  });
}

/**
 * 修改应用配置 Hook
 */
export function useUpdateConfig() {
  const queryClient = useQueryClient();
  const [saveTriggerId, setSaveTriggerId] = useState(0);

  const mutation = useMutation({
    mutationFn: async (changes: Partial<AppConfig>) => {
      const current = queryClient.getQueryData<AppConfig>(queryKeys.config());
      if (!current) return null;
      const updated = { ...current, ...changes };
      await tauriApi.setConfig(updated);
      return { updated, changes };
    },
    onSuccess: (result) => {
      if (!result) return;
      queryClient.setQueryData(queryKeys.config(), result.updated);
      if (result.changes.language) {
        useUIStore.getState().setLanguage(result.changes.language);
      }
      setSaveTriggerId((prev) => prev + 1);
    },
    onError: (err) => {
      console.error('保存设置失败:', err);
    },
  });

  const selectDir = async () => {
    try {
      const selected = await tauriApi.selectDownloadDir();
      if (selected) {
        await mutation.mutateAsync({ download_dir: selected });
      }
    } catch (err) {
      console.error('选择下载保存目录失败:', err);
    }
  };

  const openDownloadDir = async () => {
    const config = queryClient.getQueryData<AppConfig>(queryKeys.config());
    if (config?.download_dir) {
      await tauriApi.openDir(config.download_dir);
    }
  };

  const resetToDefaults = async () => {
    setDeleteFileMode('Ask');
    await mutation.mutateAsync({
      max_concurrent_tasks: 3,
      max_download_speed: 0,
      preferred_quality: 'highest',
      language: 'zh-TW',
      proxy_mode: { type: 'Direct' },
      enable_logging: false,
      auto_check_update: true,
      delete_file_on_remove: false,
    });
  };

  return {
    updateConfig: mutation.mutateAsync,
    isSaving: mutation.isPending,
    saveTriggerId,
    selectDir,
    openDownloadDir,
    resetToDefaults,
  };
}
