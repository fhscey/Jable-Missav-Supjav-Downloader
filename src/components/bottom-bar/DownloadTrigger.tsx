import { useTranslation } from 'react-i18next';
import { ArrowDownToLine } from 'lucide-react';
import { useDownloadStore } from '../../store/downloadStore';
import { useUIStore } from '../../store/uiStore';
import { BarItem } from './BarItem';

export function DownloadTrigger() {
  const { t } = useTranslation();
  const activeDrawer = useUIStore((s) => s.activeDrawer);
  const toggleDrawer = useUIStore((s) => s.toggleDrawer);
  const downloadCount = useDownloadStore((s) => s.tasks.length);

  return (
    <BarItem
      className="download-trigger"
      isActive={activeDrawer === 'downloads'}
      onClick={() => toggleDrawer('downloads')}
      title={t('bottomBar.tasks')}
      badge={
        downloadCount > 0 ? (
          <span className="pointer-events-none absolute -right-1 -top-1 grid min-w-4 place-items-center rounded-full border border-zinc-950 bg-white px-1 text-2xs leading-4 text-zinc-950 font-medium text-mono-num">
            {downloadCount}
          </span>
        ) : null
      }
    >
      <ArrowDownToLine size={14} strokeWidth={1.8} />
    </BarItem>
  );
}
