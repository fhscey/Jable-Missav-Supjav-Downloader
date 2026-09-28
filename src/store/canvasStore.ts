import { create } from 'zustand';
import { VideoInfo } from '../types';

export interface Camera {
  x: number;
  y: number;
  zoom: number;
}

export interface Viewport {
  width: number;
  height: number;
}

export interface ClusterBounds {
  minX: number;
  maxX: number;
  minY: number;
  maxY: number;
  width: number;
  height: number;
}

export interface CanvasItem<T> {
  id: string;
  x: number;
  y: number;
  width: number;
  height: number;
  slotIndex?: number;
  payload: T;
}

export type VideoItem = CanvasItem<VideoInfo>;

export const MIN_ZOOM = 0.3;
export const MAX_ZOOM = 2.0;
export const GAP = 1;
export const CELL_W = 270;
export const CELL_H = 180;

/** 把某一轴的位置夹紧到边界内；内容小于视口时居中显示 */
function clampAxis(pos: number, viewportSize: number, min: number, max: number, size: number) {
  if (size >= viewportSize) {
    return Math.max(min, Math.min(max - viewportSize, pos));
  }
  return min + (size - viewportSize) / 2;
}

/**
 * 根据相机、视口与内容包围盒，计算相机应落在的位置，
 * 使内容始终保持在可视范围内。resetCamera 与 useCanvas 的
 * clampCamera 共用这一套算法，避免逻辑重复。
 */
export function clampCameraToBounds(
  camera: Camera,
  viewport: Viewport,
  bounds: ClusterBounds | null
): { x: number; y: number } {
  if (!bounds || bounds.width <= 0 || bounds.height <= 0) {
    return { x: camera.x, y: camera.y };
  }
  const vw = viewport.width / camera.zoom;
  const vh = viewport.height / camera.zoom;
  return {
    x: clampAxis(camera.x, vw, bounds.minX, bounds.maxX, bounds.width),
    y: clampAxis(camera.y, vh, bounds.minY, bounds.maxY, bounds.height),
  };
}

export interface CanvasState {
  camera: Camera;
  viewport: Viewport;
  // 能把所有卡片刚好完整框住的最小外接矩形
  clusterBounds: ClusterBounds | null;

  setCamera: (camera: Partial<Camera> | ((prev: Camera) => Camera)) => void;
  setViewport: (viewport: Viewport) => void;
  setClusterBounds: (bounds: ClusterBounds | null) => void;
  setZoom: (targetZoom: number, anchor?: { x: number; y: number }) => void;
  resetCamera: () => void;
}

export const useCanvasStore = create<CanvasState>((set, get) => ({
  camera: getInitialCamera(),
  viewport: getInitialViewport(),
  clusterBounds: null,

  setCamera: (updater) =>
    set((state) => ({
      camera:
        typeof updater === 'function' ? updater(state.camera) : { ...state.camera, ...updater },
    })),

  setViewport: (viewport) => set({ viewport }),
  setClusterBounds: (clusterBounds) => set({ clusterBounds }),

  resetCamera: () => {
    const { viewport, clusterBounds } = get();
    const base: Camera = { x: -viewport.width / 2, y: -viewport.height / 2, zoom: 1 };
    const { x, y } = clampCameraToBounds(base, viewport, clusterBounds);
    set({ camera: { x, y, zoom: 1 } });
  },

  setZoom: (targetZoom, anchor) => {
    const { camera, viewport } = get();
    const zoom = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, targetZoom));
    if (Math.abs(zoom - camera.zoom) < 0.001) return;

    const ax = anchor?.x ?? viewport.width / 2;
    const ay = anchor?.y ?? viewport.height / 2;

    set({
      camera: {
        x: camera.x + ax * (1 / camera.zoom - 1 / zoom),
        y: camera.y + ay * (1 / camera.zoom - 1 / zoom),
        zoom,
      },
    });
  },
}));

function getInitialViewport(): Viewport {
  return {
    width: typeof window !== 'undefined' ? window.innerWidth : 1440,
    height: typeof window !== 'undefined' ? window.innerHeight : 900,
  };
}

function getInitialCamera(): Camera {
  const { width, height } = getInitialViewport();
  return { x: -width / 2, y: -height / 2, zoom: 1 };
}
