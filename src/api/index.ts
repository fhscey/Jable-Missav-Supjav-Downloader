import { downloadService } from './download';
import { mediaService } from './media';
import { systemService } from './system';

export { downloadService } from './download';
export { mediaService } from './media';
export { systemService } from './system';

/**
 * 聚合 API 契约，保持与现有业务模块向前兼容
 */
export const tauriApi = {
  // 媒体与站点解析
  fetchVideos: mediaService.fetchVideos,
  getSiteManifests: mediaService.getSiteManifests,
  searchVideos: mediaService.searchVideos,
  extractMedia: mediaService.extractMedia,
  buildPreviewStreamUrl: mediaService.buildPreviewStreamUrl,
  buildStreamUrl: mediaService.buildStreamUrl,

  // 下载管理
  getAllTasks: downloadService.getAllTasks,
  createDownloadTask: downloadService.createDownloadTask,
  startTask: downloadService.startTask,
  pauseTask: downloadService.pauseTask,
  resumeTask: downloadService.resumeTask,
  retryTask: downloadService.retryTask,
  repairTask: downloadService.repairTask,
  removeTask: downloadService.removeTask,
  subscribeProgress: downloadService.subscribeProgress,
  listenTaskStatusChanged: downloadService.listenTaskStatusChanged,

  // 系统配置与原生交互
  getConfig: systemService.getConfig,
  setConfig: systemService.setConfig,
  selectDownloadDir: systemService.selectDownloadDir,
  openDir: systemService.openDir,
  getDiskSpace: systemService.getDiskSpace,
  checkForUpdate: systemService.checkForUpdate,
  openUrl: systemService.openUrl,
};

export default tauriApi;
