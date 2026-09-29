import React, { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { ExternalLink, X } from 'lucide-react';
import { useUpdateStore } from '../../store/updateStore';
import { LogoIcon } from '../ui/LogoIcon';
import { Button } from '../ui/Button';

export const UpdateModal: React.FC = () => {
  const { t } = useTranslation();
  const showDialog = useUpdateStore((s) => s.showDialog);
  const updateInfo = useUpdateStore((s) => s.updateInfo);
  const skipVersion = useUpdateStore((s) => s.skipVersion);
  const remindLater = useUpdateStore((s) => s.remindLater);
  const updateNow = useUpdateStore((s) => s.updateNow);

  useEffect(() => {
    if (!showDialog) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        remindLater();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [showDialog, remindLater]);

  if (!showDialog || !updateInfo) {
    return null;
  }

  const latestTag = updateInfo.latestVersion.startsWith('v')
    ? updateInfo.latestVersion
    : `v${updateInfo.latestVersion}`;

  const currentTag = updateInfo.currentVersion.startsWith('v')
    ? updateInfo.currentVersion
    : `v${updateInfo.currentVersion}`;

  return (
    <div
      className="fixed inset-0 z-[190] flex items-center justify-center p-4 bg-black/45 backdrop-blur-[2px] select-none animate-in fade-in duration-150"
      onClick={remindLater}
      role="dialog"
      aria-modal="true"
    >
      <div
        style={{
          backgroundColor: 'rgba(9, 10, 14, 0.70)',
          backdropFilter: 'blur(16px)',
          WebkitBackdropFilter: 'blur(16px)',
          borderColor: 'rgba(255, 255, 255, 0.09)',
        }}
        className="relative w-full max-w-[440px] rounded-md border p-3.5 text-white shadow-[0_25px_50px_-12px_rgba(0,0,0,0.5)] animate-in zoom-in-95 duration-150 flex flex-col gap-3"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <header className="flex shrink-0 items-center justify-between pb-2.5 border-b border-white/[0.06]">
          <div className="flex flex-col gap-0.5 min-w-0">
            <div className="flex items-center gap-1.5 text-sm font-semibold text-white/90 tracking-wide">
              <LogoIcon className="size-4 shrink-0" />
              <span>{t('update.dialogTitle')}</span>
              <span className="text-2xs font-normal text-muted">{latestTag}</span>
            </div>
            <div className="text-2xs text-white/35">
              {t('update.dialogDesc', { current: currentTag, latest: latestTag })}
            </div>
          </div>
          <button
            className="btn-control !size-5.5 text-white/40 hover:text-white shrink-0"
            onClick={remindLater}
            title={t('update.remindLater')}
            aria-label={t('update.remindLater')}
          >
            <X size={12} />
          </button>
        </header>

        {/* Content Body: Release Notes */}
        <div className="space-y-1.5">
          <div className="text-section-label">{t('update.changelogTitle')}</div>
          <div className="max-h-64 overflow-y-auto overscroll-contain rounded bg-white/[0.03] p-3 text-sm text-white/80 leading-relaxed whitespace-pre-wrap select-text">
            {updateInfo.changelog?.trim() || t('update.noChangelog')}
          </div>
        </div>

        {/* Actions Footer */}
        <footer className="flex items-center justify-between pt-1">
          <Button
            variant="ghost"
            size="xs"
            onClick={() => skipVersion(updateInfo.latestVersion)}
            className="text-white/35 hover:text-rose-300 text-xs px-1"
          >
            {t('update.skipVersion')}
          </Button>

          <div className="flex items-center gap-1.5">
            <Button variant="secondary" size="xs" onClick={remindLater}>
              {t('update.remindLater')}
            </Button>
            <Button variant="primary" size="xs" onClick={updateNow}>
              <ExternalLink size={11} className="shrink-0" />
              <span>{t('update.updateNow')}</span>
            </Button>
          </div>
        </footer>
      </div>
    </div>
  );
};
