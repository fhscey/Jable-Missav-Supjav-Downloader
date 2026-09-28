import React, { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { getCurrentWindow } from '@tauri-apps/api/window';
import trafficLights from 'macos-traffic-lights';

type ButtonKind = 'close' | 'minimize' | 'maximize';

const TrafficButton: React.FC<{
  kind: ButtonKind;
  title: string;
  onClick: (e: React.MouseEvent) => void;
  groupHovered: boolean;
  onMouseEnter: () => void;
  onMouseLeave: () => void;
}> = ({ kind, title, onClick, groupHovered, onMouseEnter, onMouseLeave }) => {
  const icon = trafficLights[kind];
  const src = groupHovered ? icon.hover : icon.default;

  return (
    <button
      type="button"
      onClick={onClick}
      title={title}
      aria-label={title}
      onMouseEnter={onMouseEnter}
      onMouseLeave={onMouseLeave}
      className="relative size-3 flex items-center justify-center cursor-default outline-none"
    >
      <img
        src={src}
        alt={kind}
        className={`size-3 select-none pointer-events-none transition-transform duration-100`}
        draggable={false}
      />
    </button>
  );
};

export const WindowDragBar: React.FC = () => {
  const { t } = useTranslation();
  const win = getCurrentWindow();
  const [isFullscreen, setIsFullscreen] = useState(false);
  const [hoveredBtn, setHoveredBtn] = useState<ButtonKind | null>(null);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    win.isFullscreen().then(setIsFullscreen);
    win
      .onResized(async () => setIsFullscreen(await win.isFullscreen()))
      .then((fn) => {
        unlisten = fn;
      });
    return () => unlisten?.();
  }, []);

  const toggleFullscreen = async () => {
    const fs = await win.isFullscreen();
    await win.setFullscreen(!fs);
    setIsFullscreen(!fs);
  };

  const handleMouseDown = async (e: React.MouseEvent<HTMLDivElement>) => {
    if (e.button !== 0 || e.ctrlKey || e.metaKey) return;
    if ((e.target as HTMLElement).closest('button')) return;
    try {
      await win.startDragging();
    } catch (err) {
      console.warn('[WindowDragBar] 启动窗口拖动失败:', err);
    }
  };

  const handleDoubleClick = (e: React.MouseEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    if ((e.target as HTMLElement).closest('button')) return;
    toggleFullscreen().catch((err) => console.warn('[WindowDragBar] 双击切换全屏失败:', err));
  };

  // 红绿灯
  const buttons: { kind: ButtonKind; title: string; onClick: (e: React.MouseEvent) => void }[] = [
    {
      kind: 'close',
      title: t('common.action.close'),
      onClick: async (e) => {
        e.stopPropagation();
        win.close().catch((err) => console.warn('[WindowDragBar] 关闭窗口失败:', err));
      },
    },
    {
      kind: 'minimize',
      title: t('window.minimize'),
      onClick: async (e) => {
        e.stopPropagation();
        win.minimize().catch((err) => console.warn('[WindowDragBar] 最小化窗口失败:', err));
      },
    },
    {
      kind: 'maximize',
      title: isFullscreen ? t('window.exitFullscreen') : t('window.fullscreen'),
      onClick: async (e: React.MouseEvent) => {
        e.stopPropagation();
        if (e.altKey) {
          win.toggleMaximize().catch((err) => console.warn('[WindowDragBar] 切换全屏失败:', err));
        } else {
          toggleFullscreen().catch((err) => console.warn('[WindowDragBar] 切换全屏失败:', err));
        }
      },
    },
  ];

  return (
    <div
      data-tauri-drag-region
      onMouseDown={handleMouseDown}
      onDoubleClick={handleDoubleClick}
      className="fixed top-0 inset-x-0 h-9 z-40 select-none flex items-center pl-1.5 pr-4 pointer-events-auto"
      style={{ WebkitAppRegion: 'drag' } as React.CSSProperties}
    >
      <div
        className="group h-full flex items-center pr-3 cursor-default"
        style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}
        onMouseLeave={() => setHoveredBtn(null)}
      >
        <div className="flex items-center gap-2 px-1.5 py-1 opacity-0 transition-opacity duration-200 group-hover:opacity-100">
          {buttons.map((b) => (
            <TrafficButton
              key={b.kind}
              {...b}
              groupHovered={hoveredBtn !== null}
              onMouseEnter={() => setHoveredBtn(b.kind)}
              onMouseLeave={() => setHoveredBtn(null)}
            />
          ))}
        </div>
      </div>
    </div>
  );
};
