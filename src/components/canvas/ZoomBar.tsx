import React, { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { LocateFixed } from 'lucide-react';
import { MAX_ZOOM, MIN_ZOOM, useCanvasStore } from '../../store/canvasStore';
import { useEdgeDock } from '../../hooks/useEdgeDock';
import { cn } from '../../utils/cn';

/**
 * 极简极客 HUD 视口缩放条
 * 位于窗口顶部中央，提供微米级滑轨、一键回中与等宽百分比
 * 基于 useEdgeDock 统一管理版边感应、缩放唤起与自动收回
 */
export function ZoomBar() {
  const { t } = useTranslation();
  const zoom = useCanvasStore((s) => s.camera.zoom);
  const setZoom = useCanvasStore((s) => s.setZoom);
  const resetCamera = useCanvasStore((s) => s.resetCamera);

  const { isVisible, wakeUp, show, hide } = useEdgeDock({
    edge: 'top',
    enterThreshold: 36,
    leaveThreshold: 64,
    delayMs: 200,
  });

  // 跳过初次挂载
  const prevZoomRef = useRef(zoom);
  useEffect(() => {
    if (prevZoomRef.current !== zoom) {
      prevZoomRef.current = zoom;
      wakeUp(1500);
    }
  }, [zoom, wakeUp]);

  const handleSliderChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const val = parseFloat(e.target.value);
    setZoom(val);
  };

  const handleResetCamera = (e: React.MouseEvent) => {
    e.stopPropagation();
    resetCamera();
  };

  const handleResetZoom = (e: React.MouseEvent) => {
    e.stopPropagation();
    setZoom(1.0);
  };

  const currentPercent = Math.round(zoom * 100);
  const progressPercent = Math.max(
    0,
    Math.min(100, ((zoom - MIN_ZOOM) / (MAX_ZOOM - MIN_ZOOM)) * 100)
  );

  // 100% 缩放所在的物理基准刻线
  const defaultNotchPercent = Math.max(
    0,
    Math.min(100, ((1.0 - MIN_ZOOM) / (MAX_ZOOM - MIN_ZOOM)) * 100)
  );

  return (
    <aside
      className="fixed top-0 left-1/2 -translate-x-1/2 z-50 flex h-9 w-50 items-center justify-center pointer-events-auto"
      style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}
      onMouseEnter={show}
      onMouseLeave={hide}
    >
      <div
        className={cn(
          'hud-capsule h-6.5 gap-2 px-2 transform-gpu',
          isVisible
            ? 'opacity-100 translate-y-0 scale-100 pointer-events-auto transition-all duration-300 ease-exit]'
            : 'opacity-0 -translate-y-3 scale-[0.98] pointer-events-none transition-all duration-280 ease-exit]'
        )}
      >
        <button
          type="button"
          onClick={handleResetCamera}
          title={t('zoom.resetCamera')}
          className="group/btn flex size-5.5 items-center justify-center rounded text-secondary hover:text-primary transition-colors cursor-pointer"
        >
          <LocateFixed size={13} strokeWidth={1.9} className="transition-colors" />
        </button>

        <div className="relative flex items-center w-24 sm:w-26 h-3">
          {/* 底层槽道 */}
          <div className="track-rail">
            <div className="track-fill" style={{ width: `${progressPercent}%` }} />
          </div>

          {/* 100% 基准微刻度标志 (Notch) */}
          <span
            className="absolute top-1/2 -translate-y-1/2 -translate-x-1/2 h-1.5 w-[1px] bg-white/30 pointer-events-none"
            style={{ left: `${defaultNotchPercent}%` }}
          />

          {/* 顶层无阻尼透明原生滑块覆盖 */}
          <input
            type="range"
            min={MIN_ZOOM}
            max={MAX_ZOOM}
            step={0.01}
            value={zoom}
            onChange={handleSliderChange}
            title={t('zoom.sliderTitle', { percent: currentPercent })}
            className="absolute inset-0 size-full cursor-pointer opacity-0 z-10 appearance-none bg-transparent outline-none [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:size-3.5 [&::-webkit-slider-thumb]:bg-transparent [&::-moz-range-thumb]:size-3.5 [&::-moz-range-thumb]:bg-transparent"
          />
        </div>

        <button
          type="button"
          onClick={handleResetZoom}
          title={t('zoom.resetZoom')}
          className="group/btn flex items-center justify-center min-w-9 text-sm text-mono-num font-medium text-secondary hover:text-primary transition-colors cursor-pointer"
        >
          <span>{currentPercent}%</span>
        </button>
      </div>
    </aside>
  );
}
