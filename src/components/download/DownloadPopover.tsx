import { useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Pause, Play, RotateCcw, Trash2 } from 'lucide-react';
import { useDownloadStore } from '../../store/downloadStore';
import { DownloadItem, InlineDeleteConfirm } from './DownloadItem';
import { DownloadStats } from './DownloadStats';
import { Button } from '../ui/Button';
import { Checkbox } from '../ui/Checkbox';
import { getDeleteFileMode } from '../../hooks/queries';

interface DownloadPopoverProps {
  onClose: () => void;
}

type DownloadTab = 'active' | 'completed' | 'failed';

export function DownloadPopover({ onClose }: DownloadPopoverProps) {
  const { t } = useTranslation();
  const popoverRef = useRef<HTMLElement>(null);
  const tasks = useDownloadStore((s) => s.tasks);
  const progressMap = useDownloadStore((s) => s.progressMap);
  const initListeners = useDownloadStore((s) => s.initListeners);

  const isBatchMode = useDownloadStore((s) => s.isBatchMode);
  const selectedTaskIds = useDownloadStore((s) => s.selectedTaskIds);
  const setBatchMode = useDownloadStore((s) => s.setBatchMode);
  const selectAllTasks = useDownloadStore((s) => s.selectAllTasks);
  const clearSelection = useDownloadStore((s) => s.clearSelection);
  const batchPause = useDownloadStore((s) => s.batchPause);
  const batchResume = useDownloadStore((s) => s.batchResume);
  const batchRetry = useDownloadStore((s) => s.batchRetry);
  const batchRemove = useDownloadStore((s) => s.batchRemove);

  const [activeTab, setActiveTab] = useState<DownloadTab>('active');
  const [isDeleting, setIsDeleting] = useState(false);
  const [deleteFileChoice, setDeleteFileChoice] = useState(false);

  useEffect(() => {
    initListeners();
  }, [initListeners]);

  // 按 Esc 键关闭 Popover
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        onClose();
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [onClose]);

  const activeTasks = useMemo(
    () =>
      tasks.filter(
        (t) => t.status === 'running' || t.status === 'paused' || t.status === 'pending'
      ),
    [tasks]
  );
  const completedTasks = useMemo(
    () => tasks.filter((t) => t.status === 'completed' || t.status === 'incomplete'),
    [tasks]
  );
  const failedTasks = useMemo(() => tasks.filter((t) => t.status === 'failed'), [tasks]);

  const currentList = useMemo(() => {
    switch (activeTab) {
      case 'active':
        return activeTasks;
      case 'completed':
        return completedTasks;
      case 'failed':
        return failedTasks;
    }
  }, [activeTab, activeTasks, completedTasks, failedTasks]);

  const allSelected =
    currentList.length > 0 && currentList.every((t) => selectedTaskIds.includes(t.id));
  const isIndeterminate = selectedTaskIds.length > 0 && !allSelected;

  const toggleSelectAll = () => {
    if (allSelected) {
      clearSelection();
    } else {
      selectAllTasks(currentList.map((t) => t.id));
    }
  };

  const handleTabChange = (tab: DownloadTab) => {
    setActiveTab(tab);
    clearSelection();
    setIsDeleting(false);
  };

  const handleToggleBatchMode = () => {
    setBatchMode(!isBatchMode);
    setIsDeleting(false);
  };

  const handleBatchDeleteClick = () => {
    const mode = getDeleteFileMode();
    if (mode === 'Always') {
      void batchRemove(true);
    } else if (mode === 'Never') {
      void batchRemove(false);
    } else {
      setDeleteFileChoice(false);
      setIsDeleting(true);
    }
  };

  const handleConfirmBatchDelete = async () => {
    setIsDeleting(false);
    await batchRemove(deleteFileChoice);
  };

  return (
    <>
      {/* 隐形全屏遮罩：点击外部区域平滑收起 Popover */}
      <div className="fixed inset-0 z-[119]" onClick={onClose} aria-hidden="true" />

      <aside
        ref={popoverRef}
        className="glass-surface fixed right-5 bottom-20 z-[120] flex h-[19.5rem] w-[min(23rem,calc(100vw-2.5rem))] flex-col rounded-2xl p-2 sm:right-6 animate-in fade-in slide-in-from-bottom-2 select-none shadow-2xl"
        aria-label={t('downloads.ariaLabel')}
      >
        <DownloadStats
          activeCount={activeTasks.length}
          totalCount={tasks.length}
          onClose={onClose}
        />

        <nav
          className="flex shrink-0 items-center justify-between border-b border-white/[0.06] px-1 py-1.5"
          aria-label="Filter task status"
        >
          <div className="flex items-center gap-1">
            <button
              type="button"
              onClick={() => handleTabChange('active')}
              className={`flex items-center gap-1.5 rounded px-2 py-1 text-xs transition cursor-pointer ${
                activeTab === 'active'
                  ? 'bg-white/10 text-primary font-medium'
                  : 'text-muted hover:text-secondary'
              }`}
            >
              <span>{t('downloads.tabActive')}</span>
              <span
                className={`text-meta-sub ${activeTab === 'active' ? 'text-secondary' : 'text-sub'}`}
              >
                {activeTasks.length}
              </span>
            </button>

            <button
              type="button"
              onClick={() => handleTabChange('completed')}
              className={`flex items-center gap-1.5 rounded px-2 py-1 text-xs transition cursor-pointer ${
                activeTab === 'completed'
                  ? 'bg-white/10 text-primary font-medium'
                  : 'text-muted hover:text-secondary'
              }`}
            >
              <span>{t('downloads.tabCompleted')}</span>
              <span
                className={`text-meta-sub ${activeTab === 'completed' ? 'text-secondary' : 'text-sub'}`}
              >
                {completedTasks.length}
              </span>
            </button>

            <button
              type="button"
              onClick={() => handleTabChange('failed')}
              className={`flex items-center gap-1.5 rounded px-2 py-1 text-xs transition cursor-pointer ${
                activeTab === 'failed'
                  ? 'bg-white/10 text-primary font-medium'
                  : 'text-muted hover:text-secondary'
              }`}
            >
              <span>{t('downloads.tabFailed')}</span>
              <span
                className={`text-meta-sub ${activeTab === 'failed' ? 'text-rose-400/80 font-medium' : 'text-sub'}`}
              >
                {failedTasks.length}
              </span>
            </button>
          </div>

          {currentList.length > 0 && (
            <button
              type="button"
              onClick={handleToggleBatchMode}
              className={`rounded px-1.5 py-0.5 text-xs transition cursor-pointer ${
                isBatchMode ? 'bg-white/10 text-primary' : 'text-muted hover:text-secondary'
              }`}
            >
              {isBatchMode ? t('downloads.batch.exit') : t('downloads.batch.select')}
            </button>
          )}
        </nav>

        {isBatchMode && currentList.length > 0 && (
          <div className="flex shrink-0 items-center justify-between border-b border-white/[0.06] px-2 py-1 text-xs">
            <label className="flex items-center gap-1.5 cursor-pointer select-none text-muted hover:text-primary">
              <Checkbox
                checked={allSelected}
                indeterminate={isIndeterminate}
                onChange={toggleSelectAll}
              />
              <span>
                {selectedTaskIds.length > 0
                  ? t('downloads.batch.selectedCount', { count: selectedTaskIds.length })
                  : t('downloads.batch.selectAll')}
              </span>
            </label>

            <div className="flex items-center gap-0.5">
              {activeTab === 'active' && (
                <>
                  <Button
                    variant="ghost"
                    size="icon-xs"
                    disabled={selectedTaskIds.length === 0}
                    title={t('downloads.item.pause')}
                    aria-label={t('downloads.item.pause')}
                    onClick={() => batchPause()}
                  >
                    <Pause size={11} fill="currentColor" />
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon-xs"
                    disabled={selectedTaskIds.length === 0}
                    title={t('downloads.item.resume')}
                    aria-label={t('downloads.item.resume')}
                    onClick={() => batchResume()}
                  >
                    <Play size={11} fill="currentColor" />
                  </Button>
                </>
              )}
              {activeTab === 'failed' && (
                <Button
                  variant="ghost"
                  size="icon-xs"
                  disabled={selectedTaskIds.length === 0}
                  title={t('downloads.item.retry')}
                  aria-label={t('downloads.item.retry')}
                  onClick={() => batchRetry()}
                >
                  <RotateCcw size={11} />
                </Button>
              )}
              <Button
                variant="ghost"
                size="icon-xs"
                className="hover:text-rose-300 hover:bg-rose-500/15"
                disabled={selectedTaskIds.length === 0}
                title={t('downloads.item.delete')}
                aria-label={t('downloads.item.delete')}
                onClick={handleBatchDeleteClick}
              >
                <Trash2 size={11} />
              </Button>
            </div>
          </div>
        )}

        {isBatchMode && isDeleting && (
          <div className="px-1 pt-1">
            <InlineDeleteConfirm
              count={selectedTaskIds.length}
              deleteFileChoice={deleteFileChoice}
              onToggleChoice={setDeleteFileChoice}
              onCancel={() => setIsDeleting(false)}
              onConfirm={handleConfirmBatchDelete}
            />
          </div>
        )}

        <div data-overlay-scroll className="min-h-0 flex-1 space-y-1 overflow-y-auto px-1 py-1.5">
          {currentList.length ? (
            currentList.map((task) => (
              <DownloadItem
                key={task.id}
                task={task}
                stats={progressMap[task.id]}
                compact={activeTab !== 'active'}
              />
            ))
          ) : (
            <div className="grid min-h-32 place-items-center text-center p-4">
              <div className="space-y-1.5">
                <div className="size-1.5 rounded-full bg-white/20 mx-auto mb-2" />
                <p className="text-section-label">
                  {activeTab === 'active'
                    ? t('downloads.emptyActiveTitle')
                    : activeTab === 'completed'
                      ? t('downloads.emptyCompletedTitle')
                      : t('downloads.emptyFailedTitle')}
                </p>
                <p className="text-meta-sub">
                  {activeTab === 'active'
                    ? t('downloads.emptyActiveSubtitle')
                    : activeTab === 'completed'
                      ? t('downloads.emptyCompletedSubtitle')
                      : t('downloads.emptyFailedSubtitle')}
                </p>
              </div>
            </div>
          )}
        </div>
      </aside>
    </>
  );
}
