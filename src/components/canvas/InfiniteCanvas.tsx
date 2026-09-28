import { memo } from 'react';
import { useCanvas } from '../../hooks/useCanvas';
import { useCanvasHotkeys } from '../../hooks/useCanvasHotkeys';
import { VideoInfo } from '../../types';
import { VideoCard } from './VideoCard';
import { ZoomBar } from './ZoomBar';

export interface InfiniteCanvasProps {
  items: VideoInfo[];
  onReachBoundary?: () => void;
}

// memo 是浅比较，items 数组引用变了就会重新渲染
export const InfiniteCanvas = memo(function InfiniteCanvas({
  items,
  onReachBoundary,
}: InfiniteCanvasProps) {
  useCanvasHotkeys();

  const { viewportRef, worldRef, cameraRef, isDragging, visibleItems, handlers } = useCanvas({
    items,
    onReachBoundary,
  });
  const { x, y, zoom } = cameraRef.current;

  return (
    <div
      ref={viewportRef}
      className={`relative h-screen w-screen touch-none select-none overflow-hidden bg-black ${
        isDragging ? 'cursor-grabbing' : 'cursor-default'
      }`}
      {...handlers}
    >
      {/*
        translate3d 启动GPU加速渲染
       */}
      <div
        ref={worldRef}
        id="canvas-world"
        className="absolute left-0 top-0 size-px origin-top-left [backface-visibility:hidden] [contain:layout_style] will-change-transform"
        style={{ transform: `translate3d(${-x * zoom}px, ${-y * zoom}px, 0) scale(${zoom})` }}
      >
        {visibleItems.map((item) => (
          <div
            key={item.id}
            className="absolute left-0 top-0"
            style={{
              width: item.width,
              height: item.height,
              transform: `translate3d(${item.x}px, ${item.y}px, 0)`,
            }}
          >
            <VideoCard data={item.payload} />
          </div>
        ))}
      </div>

      <ZoomBar />
    </div>
  );
});
