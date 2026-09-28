import { useEffect, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { tauriApi } from '../../api';
import { formatBytes } from '../../utils/formatBytes';
import { cn } from '../../utils/cn';
import type { DiskSpace } from '../../types';

export interface DiskSpaceRingProps {
  /** 目标文件夹路径 */
  path?: string;
  /** 自定义样式 */
  className?: string;
  /** 环直径，默认 14px 纤细小巧 */
  size?: number;
  /** 描边线宽，默认 1.3px 极客细线 */
  strokeWidth?: number;
}

export function DiskSpaceRing({
  path,
  className,
  size = 14,
  strokeWidth = 1.3,
}: DiskSpaceRingProps) {
  const { t } = useTranslation();
  const [data, setData] = useState<DiskSpace | null>(null);
  const [loading, setLoading] = useState(false);

  const fetchSpace = useCallback(async () => {
    if (!path) return;
    setLoading(true);
    try {
      const res = await tauriApi.getDiskSpace(path);
      setData(res);
    } catch {
      // 容错保持空态
    } finally {
      setLoading(false);
    }
  }, [path]);

  useEffect(() => {
    fetchSpace();
  }, [fetchSpace]);

  if (!path) return null;

  const total = data?.total ?? 0;
  const free = data?.free ?? 0;
  const freeRatio = total > 0 ? Math.min(1, Math.max(0, free / total)) : 0;
  const freePercent = freeRatio * 100;

  // SVG 环参数
  const center = size / 2;
  const radius = (size - strokeWidth) / 2;
  const circumference = 2 * Math.PI * radius;
  const strokeDashoffset = circumference * (1 - freeRatio);

  return (
    <div className={cn('group/ring relative inline-flex items-center shrink-0', className)}>
      <button
        type="button"
        onClick={fetchSpace}
        disabled={loading}
        className="relative flex items-center justify-center rounded-full outline-none focus-visible:ring-1 focus-visible:ring-white/40 cursor-pointer"
        style={{ width: size, height: size }}
      >
        <svg
          width={size}
          height={size}
          className="-rotate-90 transform"
        >
          {/* 纯黑深色底环 */}
          <circle
            cx={center}
            cy={center}
            r={radius}
            fill="transparent"
            stroke="currentColor"
            strokeWidth={strokeWidth}
            className="text-white/15"
          />
          {/* 纯白亮色进度环 */}
          <circle
            cx={center}
            cy={center}
            r={radius}
            fill="transparent"
            stroke="currentColor"
            strokeWidth={strokeWidth}
            strokeDasharray={circumference}
            strokeDashoffset={loading ? circumference * 0.4 : strokeDashoffset}
            strokeLinecap="round"
            className={cn(
              'text-white transition-all duration-500 ease-out',
              loading && 'animate-spin origin-center text-white/60'
            )}
          />
        </svg>
      </button>

      {/* 极简浮层词条：紧凑一行，向右展开，绝不超出抽屉边界 */}
      <div className="pointer-events-none absolute left-0 bottom-full z-50 mb-1.5 whitespace-nowrap scale-95 opacity-0 transition-all duration-150 group-hover/ring:scale-100 group-hover/ring:opacity-100">
        <div className="flex items-center gap-1 rounded border border-white/10 bg-zinc-950/95 px-2 py-0.5 text-2xs font-mono shadow-xl backdrop-blur-md">
          {total > 0 ? (
            <>
              <span className="text-white/45">{t('settings.items.downloadDir.available', '可用')}</span>
              <span className="font-semibold text-white">{formatBytes(free)}</span>
              <span className="text-white/35">/</span>
              <span className="text-white/40">{formatBytes(total)}</span>
              <span className="text-white/70">({freePercent.toFixed(1)}%)</span>
            </>
          ) : (
            <span className="text-white/50">{t('common.status.loading', '检测中...')}</span>
          )}
        </div>
        {/* 指向小箭头 */}
        <div className="absolute -bottom-1 left-1.5 size-1.5 rotate-45 border-b border-r border-white/10 bg-zinc-950/95" />
      </div>
    </div>
  );
}
