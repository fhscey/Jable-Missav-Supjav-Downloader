import { useEffect } from 'react';
import { BottomBar } from '../components/bottom-bar/BottomBar';
import { ModalHost } from '../components/modals/ModalHost';
import { useDownloadStore } from '../store/downloadStore';
import { useUpdateStore } from '../store/updateStore';
import { useAppConfig, useVideoFeed } from '../hooks/queries';
import { Toast } from '../components/ui/Toast';
import { WindowDragBar } from '../components/ui/WindowDragBar';
import { cn } from '../utils/cn';
import { InfiniteCanvas } from '../components/canvas/InfiniteCanvas';

export function CanvasLayout() {
  useAppConfig();
  const { cards, fetchNexts } = useVideoFeed();

  useEffect(() => {
    useDownloadStore.getState().initListeners();
    useUpdateStore.getState().initAutoCheck();
  }, []);

  return (
    <div className={cn('app-viewport')}>
      <WindowDragBar />
      <Toast />

      <InfiniteCanvas items={cards} onReachBoundary={fetchNexts} />

      <BottomBar />

      <ModalHost />
    </div>
  );
}

export default CanvasLayout;
