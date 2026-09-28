import { invoke } from '@tauri-apps/api/core';
import { openPath, openUrl } from '@tauri-apps/plugin-opener';
import { AppConfig, UpdateInfo } from '../types';

export const systemService = {
  /**
   * 获取应用全局配置
   */
  async getConfig(): Promise<AppConfig> {
    return invoke<AppConfig>('get_config');
  },

  /**
   * 保存应用全局配置
   */
  async setConfig(config: AppConfig): Promise<void> {
    return invoke<void>('set_config', { config });
  },

  /**
   * 调起操作系统原生目录选择器
   */
  async selectDownloadDir(): Promise<string | null> {
    return invoke<string | null>('select_directory');
  },

  /**
   * 调用系统文件管理器定位到指定目录
   */
  async openDir(path: string): Promise<void> {
    await openPath(path);
  },

  /**
   * 获取指定目录所在磁盘容量信息 (总容量与剩余可用容量)
   */
  async getDiskSpace(path: string): Promise<{ total: number; free: number }> {
    return invoke<{ total: number; free: number }>('get_disk_space', { path });
  },

  /**
   * 检查应用更新
   */
  async checkForUpdate(): Promise<UpdateInfo> {
    return invoke<UpdateInfo>('check_for_update');
  },

  /**
   * 打开外部链接
   */
  async openUrl(url: string): Promise<void> {
    await openUrl(url);
  },
};
