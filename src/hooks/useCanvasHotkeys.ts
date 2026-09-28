import { useEffect } from 'react';
import { MAX_ZOOM, MIN_ZOOM, useCanvasStore } from '../store/canvasStore';
import { disconnectAll } from '../utils/disconnectAll';

interface UseCanvasHotkeysOptions {
  enabled?: boolean;
  onReset?: () => void;
  onDisconnect?: () => void;
}

// 快捷键
export function useCanvasHotkeys({
  enabled = true,
  onReset,
}: UseCanvasHotkeysOptions = {}) {
  useEffect(() => {
    if (!enabled) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      // 避免输入框、文本域内输入时误触发快捷键
      const target = e.target as HTMLElement | null;
      if (
        target instanceof HTMLInputElement ||
        target instanceof HTMLTextAreaElement ||
        target?.isContentEditable
      ) {
        return;
      }

      // Alt+Z (for Win/Linux) 或 Option+Z (for Mac)：一键断开所有连接并暂停任务
      const isOptionOrAlt = e.altKey && !e.metaKey && !e.ctrlKey;
      const isZKey = e.code === 'KeyZ' || e.key.toLowerCase() === 'z';
      if (isOptionOrAlt && isZKey) {
        e.preventDefault();
        disconnectAll();
        return;
      }

      // 数字 0：一键回中
      if (e.key === '0' && !e.metaKey && !e.ctrlKey && !e.altKey && !e.shiftKey) {
        e.preventDefault();
        if (onReset) {
          onReset();
        } else {
          useCanvasStore.getState().resetCamera();
        }
        return;
      }

      // 数字 1：还原缩放为 100%
      if (e.key === '1' && !e.metaKey && !e.ctrlKey && !e.altKey && !e.shiftKey) {
        e.preventDefault();
        useCanvasStore.getState().setZoom(1.0);
        return;
      }

      // 等号 / 加号：放大 10%
      if ((e.key === '+' || e.key === '=') && !e.metaKey && !e.ctrlKey) {
        e.preventDefault();
        const currentZoom = useCanvasStore.getState().camera.zoom;
        const nextZoom = Math.min(MAX_ZOOM, currentZoom + 0.1);
        useCanvasStore.getState().setZoom(nextZoom);
        return;
      } 

      // 减号 / 下划线：缩小 10%
      if ((e.key === '-' || e.key === '_') && !e.metaKey && !e.ctrlKey) {
        e.preventDefault();
        const currentZoom = useCanvasStore.getState().camera.zoom;
        const nextZoom = Math.max(MIN_ZOOM, currentZoom - 0.1);
        useCanvasStore.getState().setZoom(nextZoom);
        return;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [enabled, onReset]);
}

