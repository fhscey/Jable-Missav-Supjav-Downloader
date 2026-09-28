import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Compass, Settings, Unplug } from 'lucide-react';
import { useUIStore } from '../../store/uiStore';
import { useSiteStore } from '../../store/siteStore';
import { useEdgeDock } from '../../hooks/useEdgeDock';
import { disconnectAll } from '../../utils/disconnectAll';
import { BarItem } from './BarItem';
import { DownloadTrigger } from './DownloadTrigger';
import { SearchInput } from './SearchInput';
import { SortDropdown } from './SortDropdown';

/**
 * 悬浮底栏中枢 (macOS Dock 风格)
 * 纯粹的通用操作 Dock 坞，基于 useEdgeDock 实现版边感应与自动折叠
 */
export function BottomBar() {
  const { t } = useTranslation();
  const activeDrawer = useUIStore((s) => s.activeDrawer);
  const toggleDrawer = useUIStore((s) => s.toggleDrawer);
  const currentSite = useSiteStore((s) => s.currentSite);

  const [isSearchFocused, setIsSearchFocused] = useState(false);
  const [isSortOpen, setIsSortOpen] = useState(false);

  const isDrawerOpen = activeDrawer !== 'none';
  const keepAlive = isDrawerOpen || isSearchFocused || isSortOpen;

  const { isVisible, show, hide } = useEdgeDock({
    edge: 'bottom',
    keepAlive,
  });

  const handleSearchExit = () => {
    setIsSearchFocused(false);
    hide();
  };

  return (
    <footer
      className={`pointer-events-none fixed inset-x-0 bottom-0 z-[100] flex h-16 items-center justify-between px-5 select-none sm:px-6 transition-all duration-300 ease-[cubic-bezier(0.16,1,0.3,1)] ${
        isVisible ? 'translate-y-0 opacity-100' : 'translate-y-full opacity-0'
      }`}
    >
      <div className="pointer-events-auto flex items-center gap-1.5 rounded-xl p-0.5">
        {/* 站点导航 */}
        <BarItem
          isActive={activeDrawer === 'siteNav'}
          onClick={() => toggleDrawer('siteNav')}
          title={t('bottomBar.siteNav')}
        >
          <Compass size={14} strokeWidth={1.8} />
        </BarItem>
        {/* 一键断开 */}
        <BarItem
          disabled={!currentSite}
          onClick={disconnectAll}
          title={currentSite ? t('bottomBar.disconnect') : t('bottomBar.noConnection')}
          className={
            currentSite
              ? 'hover:!text-rose-400 hover:!bg-rose-500/15'
              : '!cursor-not-allowed !pointer-events-auto opacity-30'
          }
        >
          <Unplug size={14} strokeWidth={1.8} />
        </BarItem>
      </div>

      {/* 全局搜索条 */}
      <SearchInput
        isFocused={isSearchFocused}
        onFocusChange={setIsSearchFocused}
        onAutoWakeup={show}
        onExit={handleSearchExit}
      />

      {/* 排序下拉、任务列表、设置 */}
      <div className="pointer-events-auto flex items-center gap-1.5 bg-transparent">
        <SortDropdown onMenuOpenChange={setIsSortOpen} />
        <DownloadTrigger />
        <BarItem
          isActive={activeDrawer === 'settings'}
          onClick={() => toggleDrawer('settings')}
          title={t('bottomBar.settings')}
        >
          <Settings size={14} strokeWidth={1.8} />
        </BarItem>
      </div>
    </footer>
  );
}

export default BottomBar;
