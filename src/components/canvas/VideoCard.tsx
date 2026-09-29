import React, { memo, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Check, Download } from 'lucide-react';
import { tauriApi } from '../../api';
import { useDownloadStore } from '../../store/downloadStore';
import { useUIStore } from '../../store/uiStore';
import { VideoInfo } from '../../types';
import { logMediaError } from '../../utils/mediaDiagnostics';

interface VideoCardProps {
  data: VideoInfo;
}

export const VideoCard = memo(function VideoCard({ data }: VideoCardProps) {
  const { t } = useTranslation();
  const { id, title, cover_url, duration, preview_url, detail_page_url } = data;

  const [playingPreview, setPlayingPreview] = useState<boolean>(false);
  const [videoSrc, setVideoSrc] = useState<string>('');
  const hoverTimerRef = useRef<any>(null);
  const videoRef = useRef<HTMLVideoElement | null>(null);

  const isQueued = useDownloadStore((s) => s.isQueued(id));
  const createTask = useDownloadStore((s) => s.createTask);

  const enqueueDownload = async (e: React.MouseEvent<HTMLButtonElement>) => {
    e.stopPropagation();
    if (!id || isQueued) return;
    await createTask(id, detail_page_url);
  };

  // 获取预览视频地址，协议为 stream://
  const previewStreamUrl = useMemo(() => {
    if (!preview_url) return null;
    return tauriApi.buildPreviewStreamUrl(data);
  }, [data, preview_url]);

  // 悬停检测：200ms 快速意图识别，响应极其灵敏
  const handleMouseEnter = useCallback(() => {
    if (!previewStreamUrl) return;

    if (hoverTimerRef.current) clearTimeout(hoverTimerRef.current);
    hoverTimerRef.current = setTimeout(() => {
      console.info(`[VideoCard Preview] 悬停尝试加载预览: ID=${id}, URL=${previewStreamUrl}`);
      setVideoSrc(previewStreamUrl);
      setPlayingPreview(true);
    }, 200);
  }, [previewStreamUrl]);

  const handleMouseLeave = useCallback(() => {
    if (hoverTimerRef.current) {
      clearTimeout(hoverTimerRef.current);
      hoverTimerRef.current = null;
    }
    setPlayingPreview(false);
    setVideoSrc('');
    if (videoRef.current) {
      videoRef.current.pause();
      videoRef.current.removeAttribute('src');
      videoRef.current.load();
    }
  }, []);

  // 当视频源就绪时，显式调用 play() 确保 WebKit 自动播放
  useEffect(() => {
    if (playingPreview && videoRef.current && videoSrc) {
      const v = videoRef.current;
      v.muted = true;
      v.defaultMuted = true;
      const playPromise = v.play();
      if (playPromise !== undefined) {
        playPromise.catch(() => {});
      }
    }
  }, [playingPreview, videoSrc]);

  // 组件卸载清理，防止内存泄漏
  useEffect(() => {
    return () => {
      if (hoverTimerRef.current) clearTimeout(hoverTimerRef.current);
      if (videoRef.current) {
        videoRef.current.pause();
        videoRef.current.removeAttribute('src');
      }
    };
  }, []);

  return (
    <div
      className="card-base group"
      onMouseEnter={handleMouseEnter}
      onMouseLeave={handleMouseLeave}
      onClick={(e: React.MouseEvent<HTMLDivElement>) => {
        if (e.button !== 0) return;
        e.stopPropagation();
        useUIStore.getState().playCard(data);
      }}
      onContextMenu={(e: React.MouseEvent<HTMLDivElement>) => {
        e.preventDefault();
        e.stopPropagation();
        const domRect = e.currentTarget.getBoundingClientRect();
        useUIStore.getState().openDetail(data, {
          left: domRect.left,
          top: domRect.top,
          width: domRect.width,
          height: domRect.height,
          bottom: domRect.bottom,
          right: domRect.right,
        });
      }}
      onAuxClick={(e) => e.preventDefault()}
    >
      {/* 封面 */}
      <img
        src={cover_url}
        referrerPolicy="no-referrer"
        draggable={false}
        loading="lazy"
        decoding="async"
        alt=""
        className="pointer-events-none absolute inset-0 size-full object-cover select-none transition-transform duration-700 ease-out group-hover:scale-[1.035]"
        onError={(e) => {
          e.currentTarget.style.display = 'none';
        }}
      />

      {/* 悬停 Preview 视频播放层 */}
      {playingPreview && videoSrc && (
        <video
          ref={videoRef}
          src={videoSrc}
          autoPlay
          muted
          loop
          playsInline
          className="pointer-events-none absolute inset-0 z-[2] size-full object-cover bg-[#0e1015]"
          onPlay={() => {
            console.info(`[VideoCard Preview] ▶ 预览播放成功: ID=${id}`);
          }}
          onError={(e) => {
            const err = (e.currentTarget as HTMLVideoElement).error;
            logMediaError(`VideoCard Preview (ID=${id})`, videoSrc, err);
          }}
        />
      )}

      {/* 阴影渐变遮罩 */}
      <div className="pointer-events-none absolute inset-0 bg-gradient-to-b from-black/20 via-transparent via-40% to-black/85" />

      {/* 快速加入下载队列按钮 */}
      <button
        className={`btn-control absolute left-2 top-2 z-[5] group-hover:opacity-100 ${
          isQueued
            ? 'bg-black/60 text-emerald-400 opacity-90'
            : 'bg-black/35 text-white/65 opacity-0 hover:bg-black/55 hover:text-white'
        }`}
        onClick={enqueueDownload}
        title={isQueued ? t('card.alreadyInQueue') : t('card.addToQueue')}
        aria-label={isQueued ? t('card.alreadyInQueue') : t('card.addToQueue')}
      >
        {isQueued ? <Check size={13} strokeWidth={2.5} /> : <Download size={13} />}
      </button>

      {/* 底部信息区 */}
      <div className="pointer-events-none absolute inset-x-3.5 bottom-2.5 z-[4] flex items-center justify-between gap-2 text-meta text-secondary">
        {title ? (
          <span
            className="truncate max-w-[160px] text-card-title select-none"
            title={title}
          >
            {title}
          </span>
        ) : (
          <span />
        )}
        <div className="flex shrink-0 items-center gap-1.5 text-meta text-muted">
          {duration && <span>{duration}</span>}
          {playingPreview && (
            <span className="tracking-[0.12em] font-medium text-emerald-400">
              PREVIEW
            </span>
          )}
        </div>
      </div>
    </div>
  );
});
