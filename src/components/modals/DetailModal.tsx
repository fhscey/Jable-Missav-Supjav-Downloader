import React, { useEffect, useMemo, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { openUrl } from '@tauri-apps/plugin-opener';
import {
  Check,
  Download,
  ExternalLink,
  Layers,
  LoaderCircle,
  Play,
  RotateCcw,
  Tag as TagIcon,
  User,
  X,
} from 'lucide-react';
import { MediaDetail, NavItem, VideoInfo } from '../../types';
import { CardAnchorRect, useUIStore } from '../../store/uiStore';
import { useDownloadStore } from '../../store/downloadStore';
import { useSiteStore } from '../../store/siteStore';
import { useMediaDetail } from '../../hooks/queries';
import { Button } from '../ui/Button';
import { Tag } from '../ui/Tag';

interface DetailModalProps {
  card: VideoInfo;
  anchorRect?: CardAnchorRect | null;
  onClose: () => void;
  onPlay: (detail: MediaDetail) => void;
}

interface AnchorPlacement {
  placement: 'above' | 'below' | 'center';
  style: React.CSSProperties;
  arrowOffset?: number;
}

function calculateAnchorPlacement(rect: CardAnchorRect | null | undefined): AnchorPlacement {
  const modalWidth = 560;
  if (!rect || typeof window === 'undefined') {
    return {
      placement: 'center',
      style: {
        position: 'fixed',
        left: '50%',
        top: '50%',
        transform: 'translate(-50%, -50%)',
        width: `${modalWidth}px`,
        maxHeight: 'min(640px, calc(100vh - 64px))',
      },
    };
  }

  const cardCenterX = rect.left + rect.width / 2;
  const left = Math.max(
    8,
    Math.min(window.innerWidth - modalWidth - 8, cardCenterX - modalWidth / 2)
  );
  const arrowOffset = Math.max(24, Math.min(modalWidth - 28, cardCenterX - left - 6));

  const topThreshold = 40;
  const bottomThreshold = window.innerHeight - 16;
  const spaceAbove = rect.top - topThreshold;
  const spaceBelow = bottomThreshold - rect.bottom;

  if (spaceAbove >= 240 || spaceAbove >= spaceBelow) {
    const maxHeight = Math.min(600, Math.max(240, spaceAbove - 8));
    return {
      placement: 'above',
      style: {
        position: 'fixed',
        left: `${left}px`,
        bottom: `${window.innerHeight - rect.top + 8}px`,
        width: `${modalWidth}px`,
        maxHeight: `${maxHeight}px`,
      },
      arrowOffset,
    };
  } else {
    const maxHeight = Math.min(600, Math.max(240, spaceBelow - 8));
    return {
      placement: 'below',
      style: {
        position: 'fixed',
        left: `${left}px`,
        top: `${rect.bottom + 8}px`,
        width: `${modalWidth}px`,
        maxHeight: `${maxHeight}px`,
      },
      arrowOffset,
    };
  }
}

export function DetailModal({ card, anchorRect, onClose, onPlay }: DetailModalProps) {
  const { t } = useTranslation();
  const currentSite = useSiteStore((s) => s.currentSite);
  const handleSelectNavItem = useSiteStore((s) => s.handleSelectNavItem);
  const isQueued = useDownloadStore((s) => s.isQueued(card.id));
  const createTask = useDownloadStore((s) => s.createTask);
  const showToast = useUIStore((s) => s.showToast);
  const language = useUIStore((s) => s.language);

  const modalRef = useRef<HTMLDivElement>(null);
  const anchorPlacement = useMemo(() => calculateAnchorPlacement(anchorRect), [anchorRect]);

  const { detail, loading, error, refetch: fetchDetail } = useMediaDetail(card, language);


  // 按 Esc 或 画布滚动/视口尺寸变化时关闭
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };

    const handleWheel = (e: WheelEvent) => {
      if (modalRef.current && modalRef.current.contains(e.target as Node)) {
        return; // 在浮窗内容区域内滚动，不触发关闭
      }
      onClose(); // 画布产生滚动时自动收起
    };

    window.addEventListener('keydown', handleKeyDown);
    window.addEventListener('wheel', handleWheel, { passive: true });
    window.addEventListener('resize', onClose);
    return () => {
      window.removeEventListener('keydown', handleKeyDown);
      window.removeEventListener('wheel', handleWheel);
      window.removeEventListener('resize', onClose);
    };
  }, [onClose]);

  const hasStreamUrl = Boolean(!loading && detail?.stream_url);

  const handleDownload = async (e: React.MouseEvent) => {
    e.stopPropagation();
    if (!hasStreamUrl || isQueued) return;
    const success = await createTask(card.id, card.detail_page_url);
    if (success) {
      showToast(t('common.toast.addedToQueue'), 'success');
    } else {
      showToast(t('common.toast.createDownloadFailed'), 'error');
    }
  };

  const handlePlay = () => {
    if (hasStreamUrl && detail) {
      onPlay(detail);
    }
  };

  const handleItemClick = (item: NavItem) => {
    handleSelectNavItem(item);
    onClose();
  };

  const coverImage = detail?.cover_url || card.cover_url;
  const title = detail?.title || card.title;

  return (
    <>
      {/* 极轻量透明毛玻璃遮罩：点击任意处或右击均平滑收起浮窗 */}
      <div
        className="fixed inset-0 z-[150] bg-black/25 backdrop-blur-[2px] transition-opacity animate-in fade-in duration-150 select-none"
        onClick={onClose}
        onContextMenu={(e) => {
          e.preventDefault();
          onClose();
        }}
        aria-hidden="true"
      />

      {/* 悬浮于目标卡片上方/下方的透明毛玻璃卡片主体 */}
      <div
        ref={modalRef}
        style={anchorPlacement.style}
        role="dialog"
        aria-modal="true"
        onClick={(e) => e.stopPropagation()}
        onMouseDown={(e) => e.stopPropagation()}
        className="glass-surface z-[151] flex flex-col rounded-2xl shadow-[0_16px_40px_rgba(0,0,0,0.6)] text-white select-none animate-in fade-in zoom-in-95 duration-150"
      >
        {/* 指向目标卡片的微型指示箭头 */}
        {anchorPlacement.placement === 'above' && anchorPlacement.arrowOffset !== undefined && (
          <div
            className="pointer-events-none absolute -bottom-1.5 size-3 rotate-45 border-b border-r border-white/[0.1] bg-zinc-950/80 backdrop-blur-xl"
            style={{ left: `${anchorPlacement.arrowOffset}px` }}
          />
        )}
        {anchorPlacement.placement === 'below' && anchorPlacement.arrowOffset !== undefined && (
          <div
            className="pointer-events-none absolute -top-1.5 size-3 rotate-45 border-t border-l border-white/[0.1] bg-zinc-950/80 backdrop-blur-xl"
            style={{ left: `${anchorPlacement.arrowOffset}px` }}
          />
        )}

        {/* 头部精简标题栏 */}
        <div className="flex shrink-0 items-center justify-between border-b border-default px-3.5 py-2.5">
          <div className="flex items-center gap-1.5 text-section-label">
            <span className="indicator-dot" />
            <span>{t('detail.title')}</span>
          </div>
          <div className="flex items-center gap-1">
            <button onClick={onClose} className="btn-control size-6 text-muted hover:text-primary">
              <X size={13} />
            </button>
          </div>
        </div>

        {/* 可滚动内容区 */}
        <div data-overlay-scroll className="flex-1 overflow-y-auto px-4 py-3.5 space-y-3.5">
          {/* 视频缩略图与标题摘要横向组合 */}
          <div className="flex gap-3 items-start">
            <div
              className={`group/thumb relative w-32 aspect-[16/10] shrink-0 overflow-hidden rounded-lg border border-default bg-zinc-900/80 shadow-md ${
                hasStreamUrl ? 'cursor-pointer' : 'cursor-not-allowed opacity-75'
              }`}
              onClick={hasStreamUrl ? handlePlay : undefined}
              title={
                hasStreamUrl
                  ? t('detail.clickToPlay')
                  : loading
                    ? t('detail.parsingMedia')
                    : t('detail.noStreamSource')
              }
            >
              <img
                className="size-full object-cover transition-transform duration-300 group-hover/thumb:scale-105"
                src={coverImage}
                referrerPolicy="no-referrer"
                alt=""
              />
              <div className="absolute inset-0 bg-black/20 group-hover/thumb:bg-black/40 transition-colors" />
              {hasStreamUrl && (
                <div className="absolute inset-0 grid place-items-center opacity-0 group-hover/thumb:opacity-100 transition-opacity">
                  <div className="grid size-7 place-items-center rounded-full bg-white/90 text-zinc-950 shadow-md">
                    <Play size={12} className="translate-x-0.5" fill="currentColor" />
                  </div>
                </div>
              )}
              {card.duration && (
                <span className="kbd-shortcut absolute bottom-1 right-1 !bg-black/75 !text-white/90">
                  {card.duration}
                </span>
              )}
            </div>

            <div className="min-w-0 flex-1">
              <h2
                className="text-sm font-medium leading-snug text-primary line-clamp-3 select-text hover:text-white"
                title={title}
              >
                {title}
              </h2>
              {card.id && <div className="mt-1 text-meta-sub truncate">{card.id}</div>}
            </div>
          </div>

          {/* 加载中状态 */}
          {loading && (
            <div className="flex items-center gap-2 rounded-lg bg-white/[0.02] border border-subtle p-2.5 text-body-sm text-muted">
              <LoaderCircle className="animate-spin text-muted" size={13} />
              <span>{t('detail.extracting')}</span>
            </div>
          )}

          {/* 错误提示与重试 */}
          {!loading && error && (
            <div className="flex items-center justify-between rounded-lg bg-rose-500/10 border border-rose-500/20 px-3 py-2 text-xs text-rose-200">
              <span className="truncate mr-2">{error}</span>
              <Button
                variant="ghost"
                size="xs"
                onClick={fetchDetail}
                className="text-rose-200 hover:text-white"
              >
                <RotateCcw size={11} />
                {t('detail.retry')}
              </Button>
            </div>
          )}

          {/* 演员、分类、标签区域 */}
          {!loading && detail && (
            <div className="space-y-3 pt-0.5">
              {/* 演员列表 */}
              {detail.actresses && detail.actresses.length > 0 && (
                <div>
                  <div className="mb-1.5 flex items-center gap-1.5 text-section-label">
                    <User size={11} className="text-muted" />
                    <span>{t('detail.actresses')}</span>
                  </div>
                  <div className="flex flex-wrap gap-1.5">
                    {detail.actresses.map((item) => (
                      <Tag
                        key={item.url}
                        variant="default"
                        size="sm"
                        onClick={() => handleItemClick(item)}
                        title={t('detail.filterActress', { name: item.name })}
                      >
                        {item.name}
                      </Tag>
                    ))}
                  </div>
                </div>
              )}

              {/* 分类列表 */}
              {detail.categories && detail.categories.length > 0 && (
                <div>
                  <div className="mb-1.5 flex items-center gap-1.5 text-section-label">
                    <Layers size={11} className="text-muted" />
                    <span>{t('detail.categories')}</span>
                  </div>
                  <div className="flex flex-wrap gap-1.5">
                    {detail.categories.map((item) => (
                      <Tag
                        key={item.url}
                        variant="default"
                        size="sm"
                        prefixSymbol="#"
                        onClick={() => handleItemClick(item)}
                        title={t('detail.filterCategory', {
                          name: currentSite ? t(`${currentSite}.categories.${item.name}`, item.name) : item.name,
                        })}
                      >
                        {currentSite ? t(`${currentSite}.categories.${item.name}`, item.name) : item.name}
                      </Tag>
                    ))}
                  </div>
                </div>
              )}

              {/* 标签列表 */}
              {detail.tags && detail.tags.length > 0 && (
                <div>
                  <div className="mb-1.5 flex items-center gap-1.5 text-section-label">
                    <TagIcon size={11} className="text-muted" />
                    <span>{t('detail.tags')}</span>
                  </div>
                  <div className="flex flex-wrap gap-1.5">
                    {detail.tags.map((item) => (
                      <Tag
                        key={item.url}
                        variant="ghost"
                        size="sm"
                        onClick={() => handleItemClick(item)}
                        title={t('detail.filterTag', {
                          name: currentSite ? t(`${currentSite}.tags.${item.name}`, item.name) : item.name,
                        })}
                      >
                        {currentSite ? t(`${currentSite}.tags.${item.name}`, item.name) : item.name}
                      </Tag>
                    ))}
                  </div>
                </div>
              )}
            </div>
          )}
        </div>

        {/* 底部操作工具栏 */}
        <div className="flex shrink-0 items-center justify-between border-t border-white/[0.06] bg-white/[0.02] px-3.5 py-2.5 rounded-b-2xl">
          <Button variant="ghost" size="sm" onClick={() => openUrl(card.detail_page_url)}>
            <ExternalLink size={12} />
            <span>{t('detail.visitSite')}</span>
          </Button>

          <div className="flex items-center gap-1.5">
            <Button
              variant="secondary"
              size="sm"
              onClick={handleDownload}
              disabled={isQueued || !hasStreamUrl}
              title={
                isQueued
                  ? t('detail.queuedTip')
                  : !hasStreamUrl
                    ? loading
                      ? t('detail.parsingMedia')
                      : t('detail.cannotDownloadTip')
                    : t('detail.downloadTip')
              }
              className={
                isQueued ? 'border-emerald-500/30 bg-emerald-500/15 text-emerald-300' : undefined
              }
            >
              {loading ? (
                <LoaderCircle size={12} className="animate-spin text-white/40" />
              ) : isQueued ? (
                <Check size={12} strokeWidth={2.5} />
              ) : (
                <Download size={12} />
              )}
              <span>
                {loading
                  ? t('detail.parsing')
                  : isQueued
                    ? t('detail.queued')
                    : !hasStreamUrl
                      ? t('detail.cannotDownload')
                      : t('detail.download')}
              </span>
            </Button>

            <Button
              variant="primary"
              size="sm"
              onClick={handlePlay}
              disabled={!hasStreamUrl}
              title={
                !hasStreamUrl
                  ? loading
                    ? t('detail.parsingMedia')
                    : t('detail.cannotPlayTip')
                  : t('detail.playTip')
              }
            >
              <Play size={11} fill="currentColor" />
              <span>{t('detail.play')}</span>
            </Button>
          </div>
        </div>
      </div>
    </>
  );
}
