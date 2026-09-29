/**
 * 多媒体与流媒体协议诊断工具
 * 用于帮助排查 Windows 平台上的两类典型问题：
 * 1. stream:// 自定义协议在 WebView2 下无法解析 (ERR_UNKNOWN_URL_SCHEME)
 * 2. Windows 10/11 N/KN 版本缺少 Windows Media Feature Pack 导致 H.264/AAC 解码失败
 */
import { error as logError, info as logInfo, warn as logWarn } from '@tauri-apps/plugin-log';
import { useUIStore } from '../store/uiStore';

export interface MediaCapabilityReport {
  isWindows: boolean;
  userAgent: string;
  hasMediaSource: boolean;
  mp4Support: string;
  h264Support: string;
  aacSupport: string;
  tsSupport: string;
  mseH264Support: boolean;
  hasMediaFeaturePackIssue: boolean;
  suggestedStreamProtocol: string;
}

export function checkMediaCapabilities(): MediaCapabilityReport {
  const ua = typeof navigator !== 'undefined' ? navigator.userAgent : '';
  const isWindows =
    /windows|win32/i.test(ua) || (navigator as any)?.userAgentData?.platform === 'Windows';

  const video = typeof document !== 'undefined' ? document.createElement('video') : null;
  const mp4Support = video ? video.canPlayType('video/mp4') : '';
  const h264Support = video
    ? video.canPlayType('video/mp4; codecs="avc1.42E01E, mp4a.40.2"')
    : '';
  const aacSupport = video ? video.canPlayType('audio/mp4; codecs="mp4a.40.2"') : '';
  const tsSupport = video ? video.canPlayType('video/mp2t') : '';

  const hasMediaSource = typeof window !== 'undefined' && 'MediaSource' in window;
  let mseH264Support = false;
  if (hasMediaSource && window.MediaSource.isTypeSupported) {
    try {
      mseH264Support = window.MediaSource.isTypeSupported(
        'video/mp4; codecs="avc1.42E01E,mp4a.40.2"'
      );
    } catch {
      mseH264Support = false;
    }
  }

  // 若系统声称支持 mp4 但无法解码标准 H.264，或者完全不支持 MP4，通常意味着缺少 Media Feature Pack
  const hasMediaFeaturePackIssue = isWindows && (!h264Support || h264Support.length === 0);
  const suggestedStreamProtocol = isWindows ? 'http://stream.localhost' : 'stream://localhost';

  return {
    isWindows,
    userAgent: ua,
    hasMediaSource,
    mp4Support,
    h264Support,
    aacSupport,
    tsSupport,
    mseH264Support,
    hasMediaFeaturePackIssue,
    suggestedStreamProtocol,
  };
}

export function runMediaDiagnostics(): MediaCapabilityReport {
  const report = checkMediaCapabilities();

  // 仅在开发者模式 (import.meta.env.DEV) 下打印日志和弹出调试 Toast
  if (!import.meta.env.DEV) {
    return report;
  }

  console.group('%c[AVDL 媒体与流媒体协议诊断]', 'color: #3b82f6; font-weight: bold; font-size: 13px;');
  console.info(`平台: ${report.isWindows ? 'Windows' : '非 Windows (macOS/Linux)'}`);
  console.info(`User-Agent: ${report.userAgent}`);

  const diagSummary = `[MediaDiagnostics] 平台: ${report.isWindows ? 'Windows' : '非 Windows'}, H264: "${report.h264Support}", MP4: "${report.mp4Support}", MSE-H264: ${report.mseH264Support}`;
  logInfo(diagSummary).catch(() => {});

  // 1. 解码器检测 (Media Feature Pack)
  if (report.hasMediaFeaturePackIssue) {
    const warnMsg =
      '【系统警告】未检测到系统 H.264/AAC 解码器支持！若在 Windows 上，极可能是 Windows 10/11 N 或 KN 版本未安装 Media Feature Pack。';
    console.error(
      `%c[解码能力异常 - 警告] 当前系统未检测到 H.264/AAC 解码器支持！\n` +
      `这通常发生在 Windows 10/11 N 或 KN 版本中（由于未安装 Windows Media Feature Pack）。\n` +
      `如果预览和播放都黑屏报错 MEDIA_ERR_DECODE 或 MEDIA_ERR_SRC_NOT_SUPPORTED，请前往微软官网下载并安装对应版本的 Media Feature Pack。`,
      'color: #ef4444; font-weight: bold;'
    );
    logError(warnMsg).catch(() => {});

    // 在界面右下角提示用户
    setTimeout(() => {
      useUIStore.getState().showToast(warnMsg, 'error');
    }, 1500);
  } else {
    console.info(
      `%c[解码能力正常] H.264: "${report.h264Support}", MP4: "${report.mp4Support}", MSE-H264: ${report.mseH264Support}`,
      'color: #10b981; font-weight: bold;'
    );
  }

  // 2. 协议提示
  if (report.isWindows) {
    const protoMsg = `[MediaDiagnostics] Windows WebView2 自定义协议映射规则为: ${report.suggestedStreamProtocol}/... (若使用 stream:// 会报 ERR_UNKNOWN_URL_SCHEME)`;
    console.warn(
      `%c[协议诊断提示] Windows WebView2 自定义协议映射规则为: ${report.suggestedStreamProtocol}/...\n` +
      `如果请求使用 stream://localhost/...，WebView2 将直接触发 net::ERR_UNKNOWN_URL_SCHEME 导致请求无法送达 Rust 后端。`,
      'color: #f59e0b;'
    );
    logWarn(protoMsg).catch(() => {});
  }
  console.groupEnd();

  return report;
}

export function logMediaError(context: string, url: string, error: MediaError | null | undefined) {
  // 仅在开发者模式下处理并输出媒体诊断日志
  if (!import.meta.env.DEV) {
    return;
  }

  if (!error) {
    console.error(`[${context}] 媒体发生未知错误，URL: ${url}`);
    logError(`[${context}] 媒体发生未知错误，URL: ${url}`).catch(() => {});
    return;
  }

  const codeMap: Record<number, { name: string; explanation: string; suspicion: string }> = {
    1: {
      name: 'MEDIA_ERR_ABORTED',
      explanation: '用户或系统中止了媒体获取过程',
      suspicion: '用户快速切换了卡片或组件卸载',
    },
    2: {
      name: 'MEDIA_ERR_NETWORK',
      explanation: '在下载或连接时发生了网络错误',
      suspicion:
        '【高危怀疑：协议错误】WebView2 无法连接当前 URL。如果在 Windows 上使用 stream:// 协议，将必然报此错或 Code 4！同时请检查代理配置。',
    },
    3: {
      name: 'MEDIA_ERR_DECODE',
      explanation: '解码媒体资源时发生错误',
      suspicion:
        '【高危怀疑：缺少解码包】媒体流已成功拉取，但系统无法解码！若在 Windows 上，极可能是 Windows 10 N/KN 版本缺少 Media Feature Pack。',
    },
    4: {
      name: 'MEDIA_ERR_SRC_NOT_SUPPORTED',
      explanation: '媒体资源格式或协议不被当前内核支持',
      suspicion:
        '【高危怀疑：stream 协议不兼容】Windows WebView2 无法识别 stream:// 协议 (ERR_UNKNOWN_URL_SCHEME)，或者视频容器格式不被支持。',
    },
  };

  const info = codeMap[error.code] || {
    name: `UNKNOWN_CODE_${error.code}`,
    explanation: error.message || '未知错误',
    suspicion: '未知',
  };

  const logLine = `[${context}] 失败: ${info.name} (Code: ${error.code}) | URL: ${url} | 归因: ${info.suspicion}`;
  console.group(`%c[${context} 错误] ${info.name} (Code: ${error.code})`, 'color: #ef4444; font-weight: bold;');
  console.error(`请求地址: ${url}`);
  console.error(`详细消息: ${error.message || '(浏览器未提供消息)'}`);
  console.error(`官方解释: ${info.explanation}`);
  console.warn(`%c故障根因分析: ${info.suspicion}`, 'color: #f59e0b; font-weight: bold;');
  console.groupEnd();

  // 持久化输出到后端日志文件（Release 模式同样有效）
  logError(logLine).catch(() => {});

  // 针对非中止类的异常，弹出屏幕 Toast，方便不用 DevTools 也能一眼看到
  if (error.code !== 1) {
    const toastBrief =
      error.code === 3
        ? '播放失败: 解码异常 (极可能是 Windows 缺少 Media Feature Pack)'
        : error.code === 4 || error.code === 2
        ? '播放失败: 协议或网络不支持 (检查 stream:// 协议在 Windows 的兼容性)'
        : `播放失败: ${info.name}`;
    useUIStore.getState().showToast(toastBrief, 'error');
  }
}
