import { useCallback, useEffect, useRef, useState } from 'react';

export interface UseEdgeDockOptions {
  edge: 'bottom' | 'top';
  enterThreshold?: number; // 移近视口边界的触发唤出阈值 (px)
  leaveThreshold?: number; // 移出视口边界的开始收起阈值 (px)
  delayMs?: number;        // 离开感应区后的收起延迟 (ms)
  keepAlive?: boolean;     // 锁定常驻（如抽屉开启、下拉展开、正在输入等）
  autoHideOnEsc?: boolean; // 按 Esc 键时是否立即收回
}

/**
 * 屏幕版边悬浮条（HUD Dock）自适应显隐通用 Hook
 * 统一管理 macOS 风格的“靠近版边呼出、移出版边延时收回、Esc即时隐去、活动期常驻”逻辑
 */
export function useEdgeDock({
  edge,
  enterThreshold = 44,
  leaveThreshold = 88,
  delayMs = 200,
  keepAlive = false,
  autoHideOnEsc = true,
}: UseEdgeDockOptions) {
  const [isNear, setIsNear] = useState(false);
  const [isWoken, setIsWoken] = useState(false);

  const hideTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const wakeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const clearTimers = useCallback(() => {
    if (hideTimerRef.current) {
      clearTimeout(hideTimerRef.current);
      hideTimerRef.current = null;
    }
    if (wakeTimerRef.current) {
      clearTimeout(wakeTimerRef.current);
      wakeTimerRef.current = null;
    }
  }, []);

  const hide = useCallback(() => {
    clearTimers();
    setIsNear(false);
    setIsWoken(false);
  }, [clearTimers]);

  const show = useCallback(() => {
    clearTimers();
    setIsNear(true);
  }, [clearTimers]);

  const wakeUp = useCallback(
    (duration = 1500) => {
      clearTimers();
      setIsWoken(true);
      wakeTimerRef.current = setTimeout(() => {
        setIsWoken(false);
        wakeTimerRef.current = null;
      }, duration);
    },
    [clearTimers]
  );

  useEffect(() => {
    const handleWindowLeave = () => {
      if (!keepAlive) {
        hide();
      }
    };

    const handleMouseOut = (event: MouseEvent) => {
      // 当 relatedTarget / toElement 为空时，说明光标已完全移出窗口/WebView
      if (!event.relatedTarget && !(event as any).toElement) {
        handleWindowLeave();
      }
    };

    const handlePointerOut = (event: PointerEvent) => {
      if (!event.relatedTarget) {
        handleWindowLeave();
      }
    };

    const handleMouseMove = (event: MouseEvent) => {
      if (keepAlive) {
        clearTimers();
        setIsNear(true);
        return;
      }

      // 视口边界判定：当光标越过窗口顶部、底部或左右边界时，立即视为离开
      if (
        event.clientX <= 0 ||
        event.clientX >= window.innerWidth ||
        event.clientY <= 0 ||
        event.clientY >= window.innerHeight
      ) {
        handleWindowLeave();
        return;
      }

      const dist =
        edge === 'bottom'
          ? window.innerHeight - event.clientY
          : event.clientY;

      if (dist <= enterThreshold) {
        clearTimers();
        setIsNear(true);
      } else if (dist > leaveThreshold) {
        if (!hideTimerRef.current && isNear) {
          hideTimerRef.current = setTimeout(() => {
            setIsNear(false);
            hideTimerRef.current = null;
          }, delayMs);
        }
      } else {
        // 在缓冲区间停留：取消收回计时
        if (hideTimerRef.current) {
          clearTimeout(hideTimerRef.current);
          hideTimerRef.current = null;
        }
      }
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (autoHideOnEsc && event.key === 'Escape') {
        hide();
      }
    };

    window.addEventListener('mousemove', handleMouseMove);
    window.addEventListener('mouseleave', handleWindowLeave);
    window.addEventListener('mouseout', handleMouseOut);
    window.addEventListener('pointerout', handlePointerOut);
    window.addEventListener('blur', handleWindowLeave);
    document.documentElement.addEventListener('mouseleave', handleWindowLeave);
    window.addEventListener('keydown', handleKeyDown);

    return () => {
      window.removeEventListener('mousemove', handleMouseMove);
      window.removeEventListener('mouseleave', handleWindowLeave);
      window.removeEventListener('mouseout', handleMouseOut);
      window.removeEventListener('pointerout', handlePointerOut);
      window.removeEventListener('blur', handleWindowLeave);
      document.documentElement.removeEventListener('mouseleave', handleWindowLeave);
      window.removeEventListener('keydown', handleKeyDown);
      clearTimers();
    };
  }, [edge, enterThreshold, leaveThreshold, delayMs, keepAlive, autoHideOnEsc, isNear, clearTimers, hide]);

  // 兜底心跳：浮条展开期间，定时核验光标是否仍在窗口内部（解决 macOS 无边框窗口极速移出时 OS 丢事件问题）
  useEffect(() => {
    if (!isNear || keepAlive) return;

    const checkPointerInViewport = () => {
      try {
        if (!document.querySelector(':hover')) {
          hide();
        }
      } catch {}
    };

    const intervalId = setInterval(checkPointerInViewport, 200);
    return () => clearInterval(intervalId);
  }, [isNear, keepAlive, hide]);

  const isVisible = keepAlive || isNear || isWoken;

  return {
    isVisible,
    show,
    hide,
    wakeUp,
  };
}
