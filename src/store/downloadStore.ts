import { create } from 'zustand';
import { tauriApi } from '../api';
import { ProgressPayload, TaskRecord, TaskStatus } from '../types';

export interface TaskProgressStats {
  progress: number; // 0 - 100 percentage
  speedBps: number; // bytes/sec
  downloadedBytes?: number | null;
  totalBytes?: number | null;
  completedSegments?: number | null;
  totalSegments?: number | null;
}

export interface DownloadState {
  tasks: TaskRecord[];
  progressMap: Record<string, TaskProgressStats>;
  pendingTaskIds: string[];

  // 批量操作状态与动作
  isBatchMode: boolean;
  selectedTaskIds: string[];
  setBatchMode: (enabled: boolean) => void;
  toggleSelectTask: (id: string) => void;
  selectAllTasks: (ids: string[]) => void;
  clearSelection: () => void;
  batchPause: () => Promise<void>;
  batchResume: () => Promise<void>;
  batchRetry: () => Promise<void>;
  batchRemove: (deleteFile?: boolean) => Promise<void>;

  initListeners: () => Promise<void>;
  refreshTasks: () => Promise<void>;
  createTask: (id: string, url?: string) => Promise<boolean>;
  isQueued: (id?: string | null) => boolean;
  startTask: (id: string) => Promise<void>;
  pauseTask: (id: string) => Promise<void>;
  pauseAllTasks: () => Promise<void>;
  resumeTask: (id: string) => Promise<void>;
  retryTask: (id: string) => Promise<void>;
  repairTask: (id: string) => Promise<void>;
  removeTask: (id: string, deleteFile?: boolean) => Promise<void>;
}

let listenersInitialized = false;

export const useDownloadStore = create<DownloadState>((set, get) => {
  const runTaskAction = async (action: () => Promise<unknown>, errorLabel: string) => {
    try {
      await action();
      await get().refreshTasks();
    } catch (err) {
      console.error(`${errorLabel}失败:`, err);
    }
  };

  return {
    tasks: [],
    progressMap: {},
    pendingTaskIds: [],
    isBatchMode: false,
    selectedTaskIds: [],

    initListeners: async () => {
      if (listenersInitialized) return;
      listenersInitialized = true;

      // 1. 初始化拉取持久化任务记录
      await get().refreshTasks();

      // 2. 监听任务状态变迁领域事件
      await tauriApi.listenTaskStatusChanged(({ task_id, status }: { task_id: string; status: TaskStatus }) => {
        set((state) => {
          const hasTask = state.tasks.some((t) => t.id === task_id);
          const updatedTasks = hasTask
            ? state.tasks.map((t) => (t.id === task_id ? { ...t, status } : t))
            : state.tasks;

          if (!hasTask) {
            get().refreshTasks();
          }

          const prevStats = state.progressMap[task_id];
          let newProgressMap = state.progressMap;

          if (status === 'completed') {
            newProgressMap = {
              ...state.progressMap,
              [task_id]: {
                progress: 100,
                speedBps: 0,
                completedSegments: prevStats?.totalSegments ?? prevStats?.completedSegments,
                totalSegments: prevStats?.totalSegments,
                downloadedBytes: prevStats?.totalBytes ?? prevStats?.downloadedBytes,
                totalBytes: prevStats?.totalBytes,
              },
            };
          } else if (status === 'paused' || status === 'failed') {
            if (prevStats) {
              newProgressMap = {
                ...state.progressMap,
                [task_id]: {
                  ...prevStats,
                  speedBps: 0,
                },
              };
            }
          }

          return {
            tasks: updatedTasks,
            progressMap: newProgressMap,
          };
        });
      });

      // 3. 订阅高频底层进度通道（Zustand 极轻量推送，按需局部重绘）
      await tauriApi.subscribeProgress((payload: ProgressPayload) => {
        let progress = 0;
        if (payload.total_segments != null && payload.total_segments > 0) {
          const comp = payload.completed_segments ?? 0;
          progress = Math.min(100, (comp / payload.total_segments) * 100);
        } else if (payload.total_bytes != null && payload.total_bytes > 0) {
          const down = payload.downloaded_bytes ?? 0;
          progress = Math.min(100, (down / payload.total_bytes) * 100);
        }

        set((state) => ({
          progressMap: {
            ...state.progressMap,
            [payload.task_id]: {
              progress,
              speedBps: payload.speed_bps || 0,
              downloadedBytes: payload.downloaded_bytes,
              totalBytes: payload.total_bytes,
              completedSegments: payload.completed_segments,
              totalSegments: payload.total_segments,
            },
          },
        }));
      });
    },

    refreshTasks: async () => {
      try {
        const tasks = await tauriApi.getAllTasks();
        if (tasks) {
          set((state) => {
            const newProgressMap = { ...state.progressMap };
            for (const t of tasks) {
              const existing = newProgressMap[t.id];
              const isRunning = t.status === 'running';

              // 根据任务持久化字段计算真实百分比
              let progress = 0;
              if (t.status === 'completed') {
                progress = 100;
              } else if (t.total_segments != null && t.total_segments > 0 && t.completed_segments != null) {
                progress = Math.min(100, (t.completed_segments / t.total_segments) * 100);
              } else if (t.total_bytes != null && t.total_bytes > 0) {
                progress = Math.min(100, (t.downloaded_bytes / t.total_bytes) * 100);
              }

              if (!existing || !isRunning) {
                newProgressMap[t.id] = {
                  progress,
                  speedBps: isRunning ? (existing?.speedBps ?? 0) : 0,
                  downloadedBytes: t.downloaded_bytes ?? existing?.downloadedBytes,
                  totalBytes: t.total_bytes ?? existing?.totalBytes,
                  completedSegments: t.completed_segments ?? existing?.completedSegments,
                  totalSegments: t.total_segments ?? existing?.totalSegments,
                };
              }
            }
            return {
              tasks,
              progressMap: newProgressMap,
              pendingTaskIds: state.pendingTaskIds.filter(
                (pid) => !tasks.some((t) => t.id === pid || t.referer === pid)
              ),
              selectedTaskIds: state.selectedTaskIds.filter((id) => tasks.some((t) => t.id === id)),
            };
          });
        }
      } catch (err) {
        console.error('刷新下载任务列表失败:', err);
      }
    },

    /**
     * 批量操作动作
     */
    setBatchMode: (enabled) => set({ isBatchMode: enabled, selectedTaskIds: [] }),

    toggleSelectTask: (id) => {
      set((state) => ({
        selectedTaskIds: state.selectedTaskIds.includes(id)
          ? state.selectedTaskIds.filter((item) => item !== id)
          : [...state.selectedTaskIds, id],
      }));
    },

    selectAllTasks: (ids) => set({ selectedTaskIds: ids }),

    clearSelection: () => set({ selectedTaskIds: [] }),

    batchPause: async () => {
      const { selectedTaskIds } = get();
      set({ isBatchMode: false, selectedTaskIds: [] });
      await Promise.allSettled(selectedTaskIds.map((id) => tauriApi.pauseTask(id)));
      await get().refreshTasks();
    },

    batchResume: async () => {
      const { selectedTaskIds } = get();
      set({ isBatchMode: false, selectedTaskIds: [] });
      await Promise.allSettled(selectedTaskIds.map((id) => tauriApi.resumeTask(id)));
      await get().refreshTasks();
    },

    batchRetry: async () => {
      const { selectedTaskIds } = get();
      set({ isBatchMode: false, selectedTaskIds: [] });
      await Promise.allSettled(selectedTaskIds.map((id) => tauriApi.retryTask(id)));
      await get().refreshTasks();
    },

    batchRemove: async (deleteFile = false) => {
      const { selectedTaskIds } = get();
      set((state) => {
        const nextProgress = { ...state.progressMap };
        for (const id of selectedTaskIds) {
          delete nextProgress[id];
        }
        return {
          progressMap: nextProgress,
          isBatchMode: false,
          selectedTaskIds: [],
        };
      });
      await Promise.allSettled(selectedTaskIds.map((id) => tauriApi.removeTask(id, deleteFile)));
      await get().refreshTasks();
    },

    /**
     * 检查该 ID 是否已在下载队列中（包括乐观 Pending 态与已入库任务）
     */
    isQueued: (id?: string | null) => {
      if (!id) return false;
      const { tasks, pendingTaskIds } = get();
      if (pendingTaskIds.includes(id)) return true;
      return tasks.some((t) => t.id === id || t.referer === id);
    },

    /**
     * 创建下载任务（支持 0ms 乐观响应，失败自动回滚）
     */
    createTask: async (id: string, url?: string) => {
      if (!id) return false;
      const targetUrl = url || id;

      // 0ms 响应乐观变勾
      set((state) => ({
        pendingTaskIds: state.pendingTaskIds.includes(id)
          ? state.pendingTaskIds
          : [...state.pendingTaskIds, id],
      }));

      try {
        await tauriApi.createDownloadTask(targetUrl, false);
        await get().refreshTasks();
        return true;
      } catch (err) {
        console.error('创建下载任务失败:', err);
        set((state) => ({
          pendingTaskIds: state.pendingTaskIds.filter((item) => item !== id),
        }));
        return false;
      }
    },

    startTask: (id) => runTaskAction(() => tauriApi.startTask(id), '启动任务'),

    pauseTask: async (id) => {
      set((state) => {
        const stats = state.progressMap[id];
        return stats
          ? { progressMap: { ...state.progressMap, [id]: { ...stats, speedBps: 0 } } }
          : state;
      });
      await runTaskAction(() => tauriApi.pauseTask(id), '暂停任务');
    },

    pauseAllTasks: async () => {
      const runningTasks = get().tasks.filter((t) => t.status === 'running');
      if (runningTasks.length === 0) return;

      // 乐观更新：将所有进行中任务速度归零
      set((state) => {
        const nextProgress = { ...state.progressMap };
        for (const t of runningTasks) {
          if (nextProgress[t.id]) {
            nextProgress[t.id] = { ...nextProgress[t.id], speedBps: 0 };
          }
        }
        return { progressMap: nextProgress };
      });

      // 异步并发执行暂停命令
      await Promise.allSettled(
        runningTasks.map((t) =>
          tauriApi.pauseTask(t.id).catch((err) => {
            console.error(`暂停任务 ${t.id} 失败:`, err);
          })
        )
      );
      await get().refreshTasks();
    },

    resumeTask: (id) => runTaskAction(() => tauriApi.resumeTask(id), '继续任务'),

    retryTask: (id) => runTaskAction(() => tauriApi.retryTask(id), '重试任务'),

    repairTask: (id) => runTaskAction(() => tauriApi.repairTask(id), '修复任务'),

    removeTask: async (id, deleteFile) => {
      set((state) => {
        const nextProgress = { ...state.progressMap };
        delete nextProgress[id];
        return {
          progressMap: nextProgress,
          selectedTaskIds: state.selectedTaskIds.filter((item) => item !== id),
        };
      });
      await runTaskAction(() => tauriApi.removeTask(id, deleteFile), '移除任务');
    },
  };
});
