import { queryClient } from '../hooks/queries/client';
import i18n from '../i18n';
import { useCanvasStore } from '../store/canvasStore';
import { useDownloadStore } from '../store/downloadStore';
import { useSiteStore } from '../store/siteStore';
import { useUIStore } from '../store/uiStore';

/**
 * 一键断开所有连接与暂停后台任务
 *
 * 核心执行流：
 * 1. 取消正在发起的视频查询并移除 React Query 缓存，使视口卡片即刻切回 fallback cards
 * 2. 清空当前选中站点与所有分类/排序/搜索筛选
 * 3. 异步并发暂停所有正在进行的下载任务（不阻塞主 UI 响应）
 * 4. 关闭当前可能打开的抽屉面板、详情弹窗或视频播放器
 * 5. 平滑重置画布相机视野至中心原点
 * 6. 弹出轻量级 Toast 反馈
 */
export function disconnectAll() {
  const currentSite = useSiteStore.getState().currentSite;
  const runningTasks = useDownloadStore.getState().tasks.filter((t) => t.status === 'running');

  // 若当前未连接任何站点且无正在运行的任务，则无需重复执行
  if (!currentSite && runningTasks.length === 0) {
    return;
  }

  // 1. 取消与清除视频流 query 缓存
  queryClient.cancelQueries({ queryKey: ['videoFeed'] });
  queryClient.removeQueries({ queryKey: ['videoFeed'] });

  // 2. 清空站点状态（currentSite 置空，url/搜索词重置）
  useSiteStore.getState().disconnectSite();

  // 3. 异步暂停所有进行中的下载任务（不阻塞 UI 渲染）
  useDownloadStore.getState().pauseAllTasks().catch((err) => {
    console.error('暂停所有下载任务失败:', err);
  });

  // 4. 关闭当前打开的抽屉和模态框（若有）
  const uiState = useUIStore.getState();
  if (uiState.activeDrawer !== 'none') {
    uiState.closeDrawer();
  }
  if (uiState.selectedCard) {
    uiState.closeDetail();
  }
  if (uiState.playingMedia || uiState.playingCard) {
    uiState.closePlayer();
  }

  // 5. 平滑重置画布相机视野回中
  useCanvasStore.getState().resetCamera();

  // 6. 弹出轻量 Toast 提示
  const toastMsg = i18n.t('common.toast.disconnectedAll');
  uiState.showToast(toastMsg, 'info');
}

export default disconnectAll;
