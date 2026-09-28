import { Channel, invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { ProgressPayload, TaskRecord, TaskStatus } from '../types';

export const downloadService = {
  /**
   * 获取所有下载任务
   */
  async getAllTasks(): Promise<TaskRecord[]> {
    return invoke<TaskRecord[]>('get_all_tasks');
  },

  /**
   * 创建单条下载任务
   */
  async createDownloadTask(url: string, autoStart: boolean = true): Promise<TaskRecord> {
    return invoke<TaskRecord>('create_download_task', { url, autoStart });
  },

  /**
   * 启动任务
   */
  async startTask(taskId: string): Promise<void> {
    return invoke<void>('start_task', { taskId });
  },

  /**
   * 暂停任务
   */
  async pauseTask(taskId: string): Promise<void> {
    return invoke<void>('pause_task', { taskId });
  },

  /**
   * 恢复任务
   */
  async resumeTask(taskId: string): Promise<void> {
    return invoke<void>('resume_task', { taskId });
  },

  /**
   * 重试失败任务
   */
  async retryTask(taskId: string): Promise<void> {
    return invoke<void>('retry_task', { taskId });
  },

  /**
   * 修复任务 (补全缺失分片并重新封装)
   */
  async repairTask(taskId: string): Promise<void> {
    return invoke<void>('repair_task', { taskId });
  },

  /**
   * 删除任务
   */
  async removeTask(taskId: string, deleteFile: boolean = false): Promise<void> {
    return invoke<void>('remove_task', { taskId, deleteFile });
  },

  /**
   * 订阅高频下载进度通道
   */
  async subscribeProgress(
    callback: (payload: ProgressPayload) => void
  ): Promise<() => Promise<void>> {
    const chan = new Channel<ProgressPayload>(callback);
    await invoke<void>('subscribe_progress', { chan });
    return async () => {
      try {
        await invoke<void>('unsubscribe_progress');
      } catch (err) {
        console.error('Failed to unsubscribe progress:', err);
      }
    };
  },

  /**
   * 监听任务状态变迁领域事件
   */
  async listenTaskStatusChanged(
    callback: (data: { task_id: string; status: TaskStatus }) => void
  ): Promise<UnlistenFn> {
    return listen<{ task_id: string; status: TaskStatus }>('task_status_changed', (event) => {
      callback(event.payload);
    });
  },
};
