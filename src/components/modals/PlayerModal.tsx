import { useEffect, useMemo, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { renderToString } from 'react-dom/server';
import { Check, Download, LoaderCircle, RotateCcw, X } from 'lucide-react';
import Artplayer from 'artplayer';
import Hls from 'hls.js';
import { MediaDetail, VideoInfo } from '../../types';
import { tauriApi } from '../../api';
import { useDownloadStore } from '../../store/downloadStore';
import { useUIStore } from '../../store/uiStore';
import { useMediaDetail } from '../../hooks/queries';

interface PlayerModalProps {
  detail?: MediaDetail | null;
  card?: VideoInfo | null;
  onClose: () => void;
}

export function PlayerModal({ detail: initialDetail, card, onClose }: PlayerModalProps) {
  const { t } = useTranslation();
  const language = useUIStore((s) => s.language);
  const panelRef = useRef<HTMLElement>(null);

  const {
    detail: currentDetail,
    loading,
    error,
    refetch: loadMedia,
  } = useMediaDetail(card, language, {
    initialData: initialDetail,
  });

  const activeId = currentDetail?.id || card?.id || '';
  const activeDetailPageUrl = currentDetail?.detail_page_url || card?.detail_page_url || '';
  const isQueued = useDownloadStore((s) => (activeId ? s.isQueued(activeId) : false));
  const createTask = useDownloadStore((s) => s.createTask);

  useEffect(() => {
    const close = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        if (document.fullscreenElement) return;
        onClose();
      }
    };
    window.addEventListener('keydown', close);
    return () => window.removeEventListener('keydown', close);
  }, [onClose]);

  const download = async () => {
    if (isQueued || !activeId) return false;
    return await createTask(activeId, activeDetailPageUrl);
  };

  // 通过 Tauri 后端流式代理播放
  const backendStreamUrl = useMemo(() => {
    if (!currentDetail?.stream_url) return '';
    return tauriApi.buildStreamUrl(currentDetail.stream_url, currentDetail.referer);
  }, [currentDetail?.stream_url, currentDetail?.referer]);

  const title = currentDetail?.title || card?.title || t('player.defaultTitle');
  const poster = currentDetail?.cover_url || card?.cover_url || '';

  return (
    <div
      className="fixed inset-0 z-[210] grid place-items-center bg-black/60 p-4 backdrop-blur-md select-none"
      role="dialog"
      aria-modal="true"
      onMouseDown={(event) => event.target === event.currentTarget && onClose()}
    >
      <section
        ref={panelRef}
        className="glass-surface w-full max-w-6xl overflow-hidden rounded-2xl animate-in fade-in zoom-in-95 duration-200"
      >
        <header className="flex items-center justify-between border-b border-white/[0.06] bg-white/[0.015] px-4 py-2.5">
          <div className="flex items-center gap-2.5 truncate pr-4">
            <span className="relative flex size-2 shrink-0 items-center justify-center">
              <span className="absolute inline-flex size-full animate-ping rounded-full bg-white/40 opacity-75 duration-1000" />
              <span className="relative inline-flex size-1.5 rounded-full bg-white shadow-[0_0_8px_rgba(255,255,255,0.9)]" />
            </span>
            <p className="truncate text-xs font-medium text-white/90 tracking-wide">{title}</p>
          </div>
          <div className="flex items-center gap-1.5 shrink-0">
            <button
              className="btn-control !size-5.5 text-white/40 hover:text-white shrink-0"
              onClick={onClose}
              title={t('common.action.close')}
              aria-label={t('common.action.close')}
            >
              <X size={12} />
            </button>
          </div>
        </header>

        <div className="relative aspect-video w-full bg-black">
          {/* 视频加载中遮罩 */}
          {loading && (
            <div className="absolute inset-0 z-10 flex flex-col items-center justify-center bg-black/80 backdrop-blur-sm text-white">
              {poster && (
                <img
                  src={poster}
                  alt=""
                  referrerPolicy="no-referrer"
                  className="pointer-events-none absolute inset-0 size-full object-cover opacity-20 filter blur-sm"
                />
              )}
              <LoaderCircle className="animate-spin text-white/80 mb-3" size={32} />
              <p className="text-sm font-medium text-white/80">{t('player.parsingStream')}</p>
              <p className="text-xs text-white/40 mt-1">{t('player.proxyConnecting')}</p>
            </div>
          )}

          {/* 解析出错遮罩 */}
          {!loading && error && (
            <div className="absolute inset-0 z-10 flex flex-col items-center justify-center bg-black/85 text-white p-6">
              <p className="text-sm text-red-300 mb-3 text-center max-w-md">{error}</p>
              {card && (
                <button
                  onClick={loadMedia}
                  className="inline-flex items-center gap-1.5 rounded-lg bg-white/10 px-4 py-2 text-xs text-white hover:bg-white/20 transition cursor-pointer"
                >
                  <RotateCcw size={13} />
                  <span>{t('player.reparse')}</span>
                </button>
              )}
            </div>
          )}

          {/* 解析完成但未获取到流地址 */}
          {!loading && !error && !backendStreamUrl && (
            <div className="absolute inset-0 z-10 flex flex-col items-center justify-center bg-black/85 text-white p-6">
              <p className="text-sm text-red-300 mb-3 text-center max-w-md">
                {t('player.noStreamUrl')}
              </p>
              {card && (
                <button
                  onClick={loadMedia}
                  className="inline-flex items-center gap-1.5 rounded-lg bg-white/10 px-4 py-2 text-xs text-white hover:bg-white/20 transition cursor-pointer"
                >
                  <RotateCcw size={13} />
                  <span>{t('player.reparse')}</span>
                </button>
              )}
            </div>
          )}

          {/* Artplayer 播放器核心 */}
          {!loading && backendStreamUrl ? (
            <PlayerVideo
              streamUrl={backendStreamUrl}
              poster={poster}
              title={title}
              isQueued={isQueued}
              onDownload={download}
            />
          ) : null}
        </div>
      </section>
    </div>
  );
}

const DOWNLOAD_ICON_HTML = `<i class="art-icon">${renderToString(<Download size={20} strokeWidth={2} />)}</i>`;
const CHECK_ICON_HTML = `<i class="art-icon">${renderToString(<Check size={20} strokeWidth={2.5} className="text-white" />)}</i>`;

function PlayerVideo({
  streamUrl,
  poster,
  title,
  isQueued,
  onDownload,
}: {
  streamUrl: string;
  poster: string;
  title: string;
  isQueued: boolean;
  onDownload: () => Promise<boolean>;
}) {
  const { t } = useTranslation();
  const containerRef = useRef<HTMLDivElement>(null);
  const artRef = useRef<Artplayer | null>(null);
  const isQueuedRef = useRef(isQueued);
  isQueuedRef.current = isQueued;
  const onDownloadRef = useRef(onDownload);
  onDownloadRef.current = onDownload;

  useEffect(() => {
    if (!containerRef.current || !streamUrl) return;

    const isHls = streamUrl.includes('.m3u8') || decodeURIComponent(streamUrl).includes('.m3u8');

    const art = new Artplayer({
      container: containerRef.current,
      url: streamUrl,
      poster: poster || '',
      type: isHls ? 'm3u8' : 'mp4',
      customType: {
        m3u8: function playM3u8(video: HTMLVideoElement, url: string, artInstance: Artplayer) {
          if (Hls.isSupported()) {
            if ((artInstance as any).hls) {
              (artInstance as any).hls.destroy();
            }
            const hls = new Hls({
              enableWorker: true,
              lowLatencyMode: false,
            });
            hls.loadSource(url);
            hls.attachMedia(video);
            (artInstance as any).hls = hls;

            hls.on(Hls.Events.ERROR, function (_event, data) {
              if (data.fatal) {
                switch (data.type) {
                  case Hls.ErrorTypes.NETWORK_ERROR:
                    console.warn(
                      '[HLS] Network error encountered, trying to recover:',
                      data.details
                    );
                    hls.startLoad();
                    break;
                  case Hls.ErrorTypes.MEDIA_ERROR:
                    console.warn('[HLS] Media error encountered, trying to recover:', data.details);
                    hls.recoverMediaError();
                    break;
                  default:
                    console.error('[HLS] Fatal error:', data.details);
                    hls.destroy();
                    break;
                }
              }
            });

            artInstance.on('destroy', () => hls.destroy());
          } else if (video.canPlayType('application/vnd.apple.mpegurl')) {
            video.src = url;
          } else {
            artInstance.notice.show = t('player.hlsNotSupported');
          }
        },
      },
      autoplay: true,
      playbackRate: true,
      aspectRatio: true,
      setting: true,
      pip: true,
      fullscreen: true,
      fullscreenWeb: true,
      screenshot: true,
      theme: '#ffffff',
      volume: 0.8,
      hotkey: true,
      airplay: true,
      lock: true,
      playsInline: true,
      autoSize: false,
      autoMini: false,
      fastForward: true,
      controls: [
        {
          name: 'download',
          position: 'right',
          index: 25,
          html: isQueuedRef.current ? CHECK_ICON_HTML : DOWNLOAD_ICON_HTML,
          tooltip: isQueuedRef.current ? t('player.queued') : t('player.downloadVideo'),
          click: async function (this: Artplayer) {
            if (isQueuedRef.current) {
              this.notice.show = t('player.alreadyInQueue');
              return;
            }
            this.notice.show = t('player.addingToQueue');
            const success = await onDownloadRef.current();
            if (success) {
              this.notice.show = t('player.addedToQueue');
            } else {
              this.notice.show = t('player.createTaskFailed');
            }
          },
        },
      ],
    });

    artRef.current = art;

    return () => {
      artRef.current = null;
      if (art && art.destroy) {
        art.destroy(false);
      }
    };
  }, [streamUrl, poster, title, t]);

  useEffect(() => {
    if (!artRef.current) return;
    const art = artRef.current;
    if (art.controls && art.controls.update) {
      art.controls.update({
        name: 'download',
        html: isQueued ? CHECK_ICON_HTML : DOWNLOAD_ICON_HTML,
        tooltip: isQueued ? t('player.queued') : t('player.downloadVideo'),
      });
    }
  }, [isQueued, t]);

  return <div ref={containerRef} className="size-full" />;
}
