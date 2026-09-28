import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { computeLayout, getVisibleItems, Point } from '../utils/layoutEngine';
import {
  Camera,
  MAX_ZOOM,
  MIN_ZOOM,
  clampCameraToBounds,
  useCanvasStore,
  VideoItem,
} from '../store/canvasStore';
import { VideoInfo } from '../types';

export interface UseCanvasOptions<T = VideoInfo> {
  items: T[];
  onReachBoundary?: () => void;
}

interface DragState {
  isDragging: boolean;
  lastX: number;
  lastY: number;
  lastT: number;
  vx: number;
  vy: number;
}

const TRANSITION = 'transform 0.38s cubic-bezier(0.16, 1, 0.3, 1)';

/** 越界回弹阻尼：当增量会让位置更加超出边界时，予以衰减 */
function rubberBand(delta: number, pos: number, min: number, max: number, factor = 0.35) {
  if ((pos < min && delta < 0) || (pos > max && delta > 0)) return delta * factor;
  return delta;
}

export function useCanvas<T = VideoInfo>({ items, onReachBoundary }: UseCanvasOptions<T>) {
  const viewportRef = useRef<HTMLDivElement | null>(null);
  const worldRef = useRef<HTMLDivElement | null>(null);

  const viewport = useCanvasStore((s) => s.viewport);
  const storeCamera = useCanvasStore((s) => s.camera);
  const setViewport = useCanvasStore((s) => s.setViewport);
  const setClusterBounds = useCanvasStore((s) => s.setClusterBounds);

  // 高频临时相机状态（每帧更新，不走 React 渲染）
  const cameraRef = useRef<Camera>({ ...storeCamera });
  const [isDragging, setIsDragging] = useState(false);
  const [visibleCam, setVisibleCam] = useState<Camera>({ ...storeCamera });

  const dragState = useRef<DragState>({
    isDragging: false,
    lastX: 0,
    lastY: 0,
    lastT: 0,
    vx: 0,
    vy: 0,
  });
  const inertiaRafRef = useRef<number | null>(null);
  const wheelTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const boundaryCooldownRef = useRef(0);
  const lastTriggeredCountRef = useRef(0);
  // 记录用户触发越界拉取时的视口几何中心点
  const focusPointRef = useRef<Point>({ x: 0, y: 0 });

  // 1. 卡片布局与包围盒
  const { allLayoutItems, clusterBounds } = useMemo(() => {
    const { items: layoutItems, bounds } = computeLayout(
      items as any,
      focusPointRef.current
    );
    return { allLayoutItems: layoutItems, clusterBounds: bounds };
  }, [items]);

  useEffect(() => setClusterBounds(clusterBounds), [clusterBounds, setClusterBounds]);

  // 2. 应用变换（硬件加速）：smooth = true 时附加 0.38s 贝塞尔曲线过渡动画
  const applyTransform = useCallback((cam: Camera, smooth = false) => {
    if (!worldRef.current) return;
    if (smooth) {
      worldRef.current.style.transition = TRANSITION;
      setTimeout(() => {
        if (worldRef.current) worldRef.current.style.transition = '';
      }, 400);
    }
    worldRef.current.style.transform = `translate3d(${-cam.x * cam.zoom}px, ${
      -cam.y * cam.zoom
    }px, 0) scale(${cam.zoom})`;
  }, []);

  // 相机变化统一提交：写回 store 并触发可见卡片重新计算
  const commitCamera = useCallback((cam: Camera) => {
    useCanvasStore.getState().setCamera({ ...cam });
    setVisibleCam({ ...cam });
  }, []);

  const stopInertia = useCallback(() => {
    if (inertiaRafRef.current) {
      cancelAnimationFrame(inertiaRafRef.current);
      inertiaRafRef.current = null;
    }
  }, []);
  
  // 3. 边界贴合（拖拽/滚轮结束后调用）
  const clampCamera = useCallback(
    (smooth = true) => {
      const cam = cameraRef.current;
      const target = clampCameraToBounds(cam, viewport, clusterBounds);
      const isOutOfRange = Math.abs(target.x - cam.x) > 0.5 || Math.abs(target.y - cam.y) > 0.5;

      if (isOutOfRange) {
        cam.x = target.x;
        cam.y = target.y;
        applyTransform(cam, smooth);
      }
      commitCamera(cam);
    },
    [applyTransform, clusterBounds, commitCamera, viewport]
  );

  // 4. 边界触发判定：只有当用户主动手势把视口推向内容边缘/黑屏回弹区时才按需触发
  const checkBoundaryTrigger = useCallback(() => {
    if (!onReachBoundary || items.length === 0) return;
    const b = clusterBounds;
    if (!b || b.width <= 0 || b.height <= 0) return;

    const now = Date.now();
    // 基础冷却时间 2 秒，防止频繁冲击后端
    if (now - boundaryCooldownRef.current < 2000) return;

    // 若数据项数量与上次触发时相同且在 6 秒内，说明后端正在请求中，避免同状态连续重发
    const isWaitingData =
      items.length === lastTriggeredCountRef.current &&
      now - boundaryCooldownRef.current < 6000;
    if (isWaitingData) return;

    const cam = cameraRef.current;
    const vw = viewport.width / cam.zoom;
    const vh = viewport.height / cam.zoom;

    // 当内容大于视口时：只有当视口越过了已有卡片的物理边界（露出了黑屏回弹边缘），才算探索到了尽头
    // 当内容小于视口时（例如初次加载）：允许主动触发一次以补满屏幕
    const isOutRight = b.width >= vw && cam.x > b.maxX - vw;
    const isOutLeft = b.width >= vw && cam.x < b.minX;
    const isOutBottom = b.height >= vh && cam.y > b.maxY - vh;
    const isOutTop = b.height >= vh && cam.y < b.minY;
    const isUnderfilled = b.width < vw || b.height < vh;

    if (isOutRight || isOutLeft || isOutBottom || isOutTop || isUnderfilled) {
      // 记录越界时视口在世界坐标中的几何中心点（支持左上、右上、偏左、偏中等 360° 连续角度）
      focusPointRef.current = {
        x: cam.x + vw / 2,
        y: cam.y + vh / 2,
      };

      boundaryCooldownRef.current = now;
      lastTriggeredCountRef.current = items.length;
      onReachBoundary();
    }
  }, [clusterBounds, items.length, onReachBoundary, viewport]);

  // 5. 惯性滑动（带边缘阻尼）
  const startInertia = useCallback(
    (initialVx: number, initialVy: number) => {
      stopInertia();
      let vx = initialVx;
      let vy = initialVy;
      const friction = 0.93;
      let lastTime = performance.now();

      const step = () => {
        const now = performance.now();
        const dt = Math.min(32, now - lastTime);
        lastTime = now;

        const decay = Math.pow(friction, dt / 16);
        vx *= decay;
        vy *= decay;

        const cam = cameraRef.current;
        cam.x -= (vx * dt) / cam.zoom;
        cam.y -= (vy * dt) / cam.zoom;
        applyTransform(cam);

        const b = clusterBounds;
        if (b && b.width > 0 && b.height > 0) {
          const vw = viewport.width / cam.zoom;
          const vh = viewport.height / cam.zoom;
          if ((cam.x < b.minX && vx > 0) || (cam.x > b.maxX - vw && vx < 0)) {
            vx *= 0.5;
            checkBoundaryTrigger();
          }
          if ((cam.y < b.minY && vy > 0) || (cam.y > b.maxY - vh && vy < 0)) {
            vy *= 0.5;
            checkBoundaryTrigger();
          }
        }

        if (Math.hypot(vx, vy) < 0.05) {
          inertiaRafRef.current = null;
          clampCamera(true);
          return;
        }
        inertiaRafRef.current = requestAnimationFrame(step);
      };

      inertiaRafRef.current = requestAnimationFrame(step);
    },
    [applyTransform, checkBoundaryTrigger, clampCamera, clusterBounds, stopInertia, viewport]
  );

  // 5. 响应外部相机更新（快捷键回中、ZoomBar 拖动等）
  useEffect(() => {
    if (dragState.current.isDragging || inertiaRafRef.current) return;
    cameraRef.current = { ...storeCamera };
    applyTransform(storeCamera, true);
    setVisibleCam({ ...storeCamera });
  }, [storeCamera, applyTransform]);

  // 6. 监听窗口尺寸
  useEffect(() => {
    const onResize = () => setViewport({ width: window.innerWidth, height: window.innerHeight });
    window.addEventListener('resize', onResize);
    return () => window.removeEventListener('resize', onResize);
  }, [setViewport]);

  // 7. 拦截页面滚动与鼠标中键自动滚动
  useEffect(() => {
    const onWindowWheel = (e: WheelEvent) => {
      const target = e.target as HTMLElement | null;
      if (target?.closest('[data-overlay-scroll], [role="dialog"], aside')) return;
      e.preventDefault();
    };
    const preventMiddleClick = (e: MouseEvent) => {
      if (e.button === 1) e.preventDefault();
    };

    window.addEventListener('wheel', onWindowWheel, { passive: false });
    window.addEventListener('mousedown', preventMiddleClick, { passive: false });
    window.addEventListener('auxclick', preventMiddleClick, { passive: false });
    return () => {
      window.removeEventListener('wheel', onWindowWheel);
      window.removeEventListener('mousedown', preventMiddleClick);
      window.removeEventListener('auxclick', preventMiddleClick);
    };
  }, []);

  // 8. 拖拽手势（PointerCapture）
  const handlePointerDown = useCallback(
    (e: React.PointerEvent<HTMLDivElement>) => {
      const isMiddle = e.button === 1;
      const isLeftOnBackground =
        e.button === 0 && (e.target === viewportRef.current || e.target === worldRef.current);
      if (!isMiddle && !isLeftOnBackground) return;

      e.preventDefault();
      try {
        (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
      } catch {}

      stopInertia();
      if (worldRef.current) worldRef.current.style.transition = '';

      dragState.current = {
        isDragging: true,
        lastX: e.clientX,
        lastY: e.clientY,
        lastT: performance.now(),
        vx: 0,
        vy: 0,
      };
      setIsDragging(true);
    },
    [stopInertia]
  );

  const handlePointerMove = useCallback(
    (e: React.PointerEvent<HTMLDivElement>) => {
      if (!dragState.current.isDragging) return;
      const now = performance.now();
      const dx = e.clientX - dragState.current.lastX;
      const dy = e.clientY - dragState.current.lastY;
      const dt = Math.max(1, now - dragState.current.lastT);

      const cam = cameraRef.current;
      const b = clusterBounds;
      let addX = -dx / cam.zoom;
      let addY = -dy / cam.zoom;

      if (b && b.width > 0 && b.height > 0) {
        const vw = viewport.width / cam.zoom;
        const vh = viewport.height / cam.zoom;
        addX = rubberBand(addX, cam.x, b.minX, b.maxX - vw);
        addY = rubberBand(addY, cam.y, b.minY, b.maxY - vh);
      }

      cam.x += addX;
      cam.y += addY;
      applyTransform(cam);
      checkBoundaryTrigger();

      dragState.current.vx = dx / dt;
      dragState.current.vy = dy / dt;
      dragState.current.lastX = e.clientX;
      dragState.current.lastY = e.clientY;
      dragState.current.lastT = now;

      // 位移累计达到 200px 才触发一次可见卡片重算，避免拖拽时频繁渲染
      if (Math.abs(cam.x - visibleCam.x) > 200 || Math.abs(cam.y - visibleCam.y) > 200) {
        setVisibleCam({ ...cam });
      }
    },
    [applyTransform, checkBoundaryTrigger, clusterBounds, viewport, visibleCam]
  );

  const handlePointerUp = useCallback(
    (e: React.PointerEvent<HTMLDivElement>) => {
      if (!dragState.current.isDragging) return;
      dragState.current.isDragging = false;
      setIsDragging(false);

      try {
        (e.currentTarget as HTMLElement).releasePointerCapture(e.pointerId);
      } catch {}

      const speed = Math.hypot(dragState.current.vx, dragState.current.vy);
      if (speed > 0.15) startInertia(dragState.current.vx, dragState.current.vy);
      else clampCamera(true);
    },
    [clampCamera, startInertia]
  );

  // 9. 滚轮：Ctrl/⌘ + 滚轮或触控板捏合缩放，否则平移
  const handleWheel = useCallback(
    (e: React.WheelEvent<HTMLDivElement>) => {
      stopInertia();
      if (worldRef.current) worldRef.current.style.transition = '';

      const cam = cameraRef.current;

      if (e.ctrlKey || e.metaKey) {
        const factor = Math.min(1.3, Math.max(0.7, Math.exp(-e.deltaY * 0.0025)));
        const newZoom = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, cam.zoom * factor));
        if (Math.abs(newZoom - cam.zoom) <= 0.0001) return;

        const rect = viewportRef.current?.getBoundingClientRect();
        const mouseX = rect ? e.clientX - rect.left : e.clientX;
        const mouseY = rect ? e.clientY - rect.top : e.clientY;

        cam.x += mouseX * (1 / cam.zoom - 1 / newZoom);
        cam.y += mouseY * (1 / cam.zoom - 1 / newZoom);
        cam.zoom = newZoom;

        applyTransform(cam);
        commitCamera(cam);
      } else {
        let addX = e.deltaX / cam.zoom;
        let addY = e.deltaY / cam.zoom;

        const b = clusterBounds;
        if (b && b.width > 0 && b.height > 0) {
          const vw = viewport.width / cam.zoom;
          const vh = viewport.height / cam.zoom;
          addX = rubberBand(addX, cam.x, b.minX, b.maxX - vw);
          addY = rubberBand(addY, cam.y, b.minY, b.maxY - vh);
        }

        cam.x += addX;
        cam.y += addY;
        applyTransform(cam);
        checkBoundaryTrigger();

        if (Math.abs(cam.x - visibleCam.x) > 200 || Math.abs(cam.y - visibleCam.y) > 200) {
          setVisibleCam({ ...cam });
        }
      }

      if (wheelTimerRef.current) clearTimeout(wheelTimerRef.current);
      wheelTimerRef.current = setTimeout(() => clampCamera(true), 200);
    },
    [applyTransform, checkBoundaryTrigger, clampCamera, clusterBounds, commitCamera, stopInertia, viewport, visibleCam]
  );

  // 10. 视口虚拟化：只渲染可见范围内的卡片
  const visibleItems = useMemo(() => {
    if (allLayoutItems.length === 0) return [] as VideoItem[];
    return getVisibleItems(visibleCam, viewport, allLayoutItems);
  }, [allLayoutItems, viewport, visibleCam]);

  return {
    viewportRef,
    worldRef,
    cameraRef,
    isDragging,
    visibleItems,
    handlers: {
      onPointerDown: handlePointerDown,
      onPointerMove: handlePointerMove,
      onPointerUp: handlePointerUp,
      onPointerCancel: handlePointerUp,
      onWheel: handleWheel,
    },
  };
}
