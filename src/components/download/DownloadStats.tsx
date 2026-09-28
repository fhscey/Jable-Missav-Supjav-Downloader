import { useTranslation } from 'react-i18next';
import { Download, X } from 'lucide-react';

interface DownloadStatsProps {
  activeCount: number;
  totalCount: number;
  onClose: () => void;
}

export function DownloadStats({ activeCount, totalCount, onClose }: DownloadStatsProps) {
  const { t } = useTranslation();
  return (
    <header className="flex shrink-0 items-center justify-between border-b border-default px-2.5 py-2">
      <div className="flex items-center gap-2">
        <Download className="size-3 text-secondary" />
        <span className="text-section-title">
          {t('downloads.transfers')}
        </span>
      </div>

      <div className="flex items-center gap-2">
        <div className="flex items-center gap-1 text-meta">
          {activeCount > 0 ? (
            <span className="text-primary font-medium">{activeCount} {t('downloads.active')}</span>
          ) : (
            <span>{t('downloads.idle')}</span>
          )}
          <span className="text-sub">/</span>
          <span>{totalCount} {t('downloads.total')}</span>
        </div>
        <button
          type="button"
          className="btn-control !size-6 text-muted hover:text-primary"
          onClick={onClose}
          title={t('common.action.close')}
          aria-label={t('common.action.close')}
        >
          <X size={13} />
        </button>
      </div>
    </header>
  );
}
