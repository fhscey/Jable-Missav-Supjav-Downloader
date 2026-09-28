import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Folder, Pause, Play, RotateCcw, Trash2, Wrench } from 'lucide-react';
import { TaskProgressStats, useDownloadStore } from '../../store/downloadStore';
import { getDeleteFileMode, useAppConfig } from '../../hooks/queries';
import { TaskRecord, TaskStatus } from '../../types';
import { formatBytes, formatSpeed } from '../../utils/formatBytes';
import { Button } from '../ui/Button';
import { Checkbox } from '../ui/Checkbox';
import { tauriApi } from '../../api';

export const statusBadge: Record<TaskStatus, { code: string; dotClass: string }> = {
  running: {
    code: 'RUN',
    dotClass: 'bg-white shadow-[0_0_6px_rgba(255,255,255,0.9)] animate-pulse',
  },
  paused: {
    code: 'PAUSE',
    dotClass: 'bg-white/35',
  },
  completed: {
    code: 'DONE',
    dotClass: 'bg-white/70',
  },
  failed: {
    code: 'ERR',
    dotClass: 'bg-rose-400 shadow-[0_0_6px_rgba(244,63,94,0.6)]',
  },
  pending: {
    code: 'WAIT',
    dotClass: 'bg-white/20 border border-white/40',
  },
  incomplete: {
    code: 'INCOMP',
    dotClass: 'bg-amber-300 shadow-[0_0_6px_rgba(252,211,77,0.5)]',
  },
};

interface DownloadItemProps {
  task: TaskRecord;
  stats?: TaskProgressStats;
  compact?: boolean;
}

export function DownloadItem({
  task,
  stats,
  compact = false,
}: DownloadItemProps) {
  const { t } = useTranslation();
  const { startTask, pauseTask, resumeTask, retryTask, repairTask, removeTask } =
    useDownloadStore();
  const isBatchMode = useDownloadStore((s) => s.isBatchMode);
  const isSelected = useDownloadStore((s) => s.selectedTaskIds.includes(task.id));
  const toggleSelectTask = useDownloadStore((s) => s.toggleSelectTask);
  const { data: config } = useAppConfig();
  const deleteFileMode = getDeleteFileMode();

  const [isDeleting, setIsDeleting] = useState(false);
  const [deleteFileChoice, setDeleteFileChoice] = useState(false);

  const badge = statusBadge[task.status] || statusBadge.pending;

  const handleDeleteClick = () => {
    if (deleteFileMode === 'Always') {
      removeTask(task.id, true);
    } else if (deleteFileMode === 'Never') {
      removeTask(task.id, false);
    } else {
      setIsDeleting(true);
      setDeleteFileChoice(false);
    }
  };

  const handleConfirmDelete = async () => {
    await removeTask(task.id, deleteFileChoice);
    setIsDeleting(false);
  };

  const handleOpenFolder = async (e: React.MouseEvent) => {
    e.stopPropagation();
    const dir = task.save_dir || config?.download_dir;
    if (dir) {
      try {
        await tauriApi.openDir(dir);
      } catch (err) {
        console.error('打开保存目录失败:', err);
      }
    }
  };

  if (compact) {
    return (
      <article className="group relative rounded-md px-2 py-1.5 transition duration-150 hover:bg-white/[0.04]">
        <div className="flex items-center justify-between gap-4">
          {/* 左侧状态点 + 标题 */}
          <div className="flex min-w-0 flex-1 items-center gap-2">
            {isBatchMode && (
              <Checkbox
                checked={isSelected}
                onChange={() => toggleSelectTask(task.id)}
              />
            )}
            <span className={`size-1.5 shrink-0 rounded-full ${badge.dotClass}`} />
            <p
              className="min-w-0 flex-1 truncate text-card-title"
              title={task.title || task.id}
            >
              {task.title || task.id}
            </p>
          </div>

          <div className="flex shrink-0 items-center gap-1.5">
            {task.status === 'incomplete' && (
              <span className="text-meta-sub text-amber-300">INCOMPLETE</span>
            )}

            <time className="hidden sm:inline text-meta-sub">
              {new Date(task.created_at * 1000).toLocaleTimeString([], {
                hour: '2-digit',
                minute: '2-digit',
              })}
            </time>

            {!isBatchMode && (
              <div className="flex items-center gap-0.5">
                {task.status === 'incomplete' && (
                  <Button
                    variant="ghost"
                    size="icon-xs"
                    title={t('downloads.item.repair')}
                    aria-label={t('downloads.item.repair')}
                    onClick={() => repairTask(task.id)}
                  >
                    <Wrench size={11} className="text-amber-300/90" />
                  </Button>
                )}
                <Button
                  variant="ghost"
                  size="icon-xs"
                  title={t('downloads.item.retry')}
                  aria-label={t('downloads.item.retry')}
                  onClick={() => retryTask(task.id)}
                >
                  <RotateCcw size={11} />
                </Button>
                <Button
                  variant="ghost"
                  size="icon-xs"
                  title={t('downloads.item.openFolder')}
                  aria-label={t('downloads.item.openFolder')}
                  onClick={handleOpenFolder}
                >
                  <Folder size={11} />
                </Button>
                <Button
                  variant="ghost"
                  size="icon-xs"
                  className="hover:text-rose-300 hover:bg-rose-500/15"
                  title={t('downloads.item.delete')}
                  aria-label={t('downloads.item.delete')}
                  onClick={handleDeleteClick}
                >
                  <Trash2 size={11} />
                </Button>
              </div>
            )}
          </div>
        </div>

        {isDeleting && (
          <InlineDeleteConfirm
            deleteFileChoice={deleteFileChoice}
            onToggleChoice={setDeleteFileChoice}
            onCancel={() => setIsDeleting(false)}
            onConfirm={handleConfirmDelete}
          />
        )}
      </article>
    );
  }

  const progressPercent =
    task.status === 'running' || task.status === 'paused' || task.status === 'incomplete'
      ? (stats?.progress ?? 0)
      : 0;

  return (
    <article className="group relative rounded-md p-2 transition duration-150 hover:border-default hover:bg-white/[0.04]">
      <div className="mb-1.5 flex items-center justify-between gap-4">
        <div className="flex min-w-0 flex-1 items-center gap-2">
          {isBatchMode && (
            <Checkbox
              checked={isSelected}
              onChange={() => toggleSelectTask(task.id)}
            />
          )}
          <p className="min-w-0 flex-1 truncate text-card-title" title={task.title || task.id}>
            {task.title || task.id}
          </p>
        </div>

        <div className="flex shrink-0 items-center gap-1.5">
          <span className={`size-1.5 rounded-full ${badge.dotClass}`} />
          <span className="text-section-label">
            {badge.code}
          </span>
        </div>
      </div>

      <div className="track-rail my-2">
        <div
          className={`track-fill ${task.status === 'paused' ? '!bg-white/40' : ''}`}
          style={{ width: `${progressPercent}%` }}
        />
      </div>

      <div className="flex items-center justify-between text-meta">
        <div className="flex items-center gap-1.5">
          <span className="text-secondary">
            {progressPercent > 0 ? `${progressPercent.toFixed(1)}%` : '0.0%'}
          </span>
          <span className="text-sub">·</span>
          <span>
            {stats?.totalSegments
              ? `${stats.completedSegments ?? 0}/${stats.totalSegments} SEGS`
              : stats?.totalBytes
                ? `${formatBytes(stats.downloadedBytes)} / ${formatBytes(stats.totalBytes)}`
                : formatBytes(stats?.downloadedBytes)}
          </span>
        </div>

        {task.status === 'running' && (
          <span className="text-primary font-medium text-mono-num">{formatSpeed(stats?.speedBps ?? 0)}</span>
        )}
      </div>

      <footer className="mt-2 flex items-center justify-between">
        <time className="text-meta-sub">
          {new Date(task.created_at * 1000).toLocaleTimeString([], {
            hour: '2-digit',
            minute: '2-digit',
            second: '2-digit',
          })}
        </time>

        {!isBatchMode && (
          <div className="flex items-center gap-0.5">
            {task.status === 'pending' && (
              <Button
                variant="ghost"
                size="icon-xs"
                title={t('downloads.item.start')}
                aria-label={t('downloads.item.start')}
                onClick={() => startTask(task.id)}
              >
                <Play size={11} fill="currentColor" />
              </Button>
            )}
            {task.status === 'running' && (
              <Button
                variant="ghost"
                size="icon-xs"
                title={t('downloads.item.pause')}
                aria-label={t('downloads.item.pause')}
                onClick={() => pauseTask(task.id)}
              >
                <Pause size={11} fill="currentColor" />
              </Button>
            )}
            {task.status === 'paused' && (
              <Button
                variant="ghost"
                size="icon-xs"
                title={t('downloads.item.resume')}
                aria-label={t('downloads.item.resume')}
                onClick={() => resumeTask(task.id)}
              >
                <Play size={11} fill="currentColor" />
              </Button>
            )}
            {/* 打开的是 config.save_dir，但未完成时的文件夹名称是 task.id */}
            <Button
              variant="ghost"
              size="icon-xs"
              title={task.id}
              aria-label={t('downloads.item.openFolder')}
              onClick={handleOpenFolder}
            >
              <Folder size={11} />
            </Button>
            <Button
              variant="ghost"
              size="icon-xs"
              className="hover:text-rose-300 hover:bg-rose-500/15"
              title={t('downloads.item.delete')}
              aria-label={t('downloads.item.delete')}
              onClick={handleDeleteClick}
            >
              <Trash2 size={11} />
            </Button>
          </div>
        )}
      </footer>

      {/* 行内删除确认 */}
      {isDeleting && (
        <InlineDeleteConfirm
          deleteFileChoice={deleteFileChoice}
          onToggleChoice={setDeleteFileChoice}
          onCancel={() => setIsDeleting(false)}
          onConfirm={handleConfirmDelete}
        />
      )}
    </article>
  );
}

export function InlineDeleteConfirm({
  deleteFileChoice,
  onToggleChoice,
  onCancel,
  onConfirm,
  count = 1,
}: {
  deleteFileChoice: boolean;
  onToggleChoice: (checked: boolean) => void;
  onCancel: () => void;
  onConfirm: () => void;
  count?: number;
}) {
  const { t } = useTranslation();
  return (
    <div className="mt-1.5 rounded-md border border-default bg-black/60 p-2 text-xs backdrop-blur-sm animate-in fade-in duration-100">
      <div className="flex items-center justify-between text-meta text-primary">
        <span>
          {count > 1
            ? t('downloads.item.confirmDeletePlural', { count })
            : t('downloads.item.confirmDelete')}
        </span>
        <span className="text-rose-400 text-meta-sub">{t('downloads.item.irreversible')}</span>
      </div>

      <div className="mt-2 flex items-center justify-between border-t border-subtle pt-1.5">
        <label className="flex cursor-pointer select-none items-center gap-1.5 text-meta text-muted hover:text-primary transition-colors">
          <Checkbox
            checked={deleteFileChoice}
            onChange={onToggleChoice}
          />
          <span>{t('downloads.item.purgeDiskFile')}</span>
        </label>

        <div className="flex items-center gap-1.5">
          <Button variant="ghost" size="xs" onClick={onCancel}>
            {t('downloads.item.cancel')}
          </Button>
          <Button variant="danger" size="xs" onClick={onConfirm}>
            {t('downloads.item.deleteBtn')}
          </Button>
        </div>
      </div>
    </div>
  );
}
