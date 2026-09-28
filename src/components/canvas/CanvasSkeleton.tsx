import { memo } from 'react';
import { CELL_H, CELL_W } from '../../store/canvasStore';

interface CanvasSkeletonProps {
  x: number;
  y: number;
  width?: number;
  height?: number;
}

export const CanvasSkeleton = memo(function CanvasSkeleton({
  x,
  y,
  width = CELL_W,
  height = CELL_H,
}: CanvasSkeletonProps) {
  return (
    <div
      className="pointer-events-none absolute select-none box-border"
      style={{
        transform: `translate3d(${x}px, ${y}px, 0)`,
        width: `${width}px`,
        height: `${height}px`,
      }}
    >
      <div className="relative flex size-full flex-col justify-end overflow-hidden rounded-xl border border-white/[0.06] bg-white/[0.03] p-4 box-border animate-pulse">
        <div className="mb-2 h-3.5 w-3/4 rounded bg-white/[0.08]" />
        <div className="h-2.5 w-2/5 rounded bg-white/[0.05]" />
      </div>
    </div>
  );
});
