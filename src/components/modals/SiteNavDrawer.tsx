import { useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ChevronDown, Command, Flame, Layers, Search, Tag, X } from 'lucide-react';
import { useSiteStore } from '../../store/siteStore';
import { useUIStore } from '../../store/uiStore';
import { useActiveManifest, useSiteManifests } from '../../hooks/queries';
import { NavItem, TagGroup } from '../../types';
import { Drawer } from '../ui/Drawer';
import { LogoIcon } from '../ui/AvLogoIcon';
import pkg from '../../../package.json';
import { useUpdateStore } from '../../store/updateStore';

interface SiteNavDrawerProps {
  onClose: () => void;
}

export function SiteNavDrawer({ onClose }: SiteNavDrawerProps) {
  const { t } = useTranslation();
  const currentSite = useSiteStore((s) => s.currentSite);
  const { data: manifests = [] } = useSiteManifests();
  const activeManifest = useActiveManifest();
  const handleSelectSite = useSiteStore((s) => s.handleSelectSite);
  const handleSelectNavItem = useSiteStore((s) => s.handleSelectNavItem);
  const collapsedTagGroups = useUIStore((s) => s.collapsedTagGroups);
  const toggleTagGroupCollapsed = useUIStore((s) => s.toggleTagGroupCollapsed);
  const hasUpdate = useUpdateStore((s) => s.hasUpdate);
  const openUpdateDialog = useUpdateStore((s) => s.openDialog);

  const [searchQuery, setSearchQuery] = useState('');
  const searchInputRef = useRef<HTMLInputElement>(null);

  // Focus search input on Cmd+K or Ctrl+K
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault();
        searchInputRef.current?.focus();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  const tags = useMemo(() => activeManifest?.tags || [], [activeManifest?.tags]);
  const quickLinks = useMemo(
    () => activeManifest?.quick_links || [],
    [activeManifest?.quick_links]
  );
  const categories = useMemo(() => activeManifest?.categories || [], [activeManifest?.categories]);

  const totalTagsCount = useMemo(() => {
    return tags.reduce((acc, g) => acc + (g?.items?.length || 0), 0);
  }, [tags]);

  // Jump to tag: filter tag groups within the drawer
  const filteredTagGroups = useMemo(() => {
    const q = searchQuery.trim().toLowerCase();
    if (!q) return tags;
    return tags
      .map((group) => ({
        ...group,
        items: (group?.items || []).filter((item) => {
          const raw = item?.name?.toLowerCase() || '';
          const localized = t(`${currentSite}.tags.${item.name}`, item.name).toLowerCase();
          return raw.includes(q) || localized.includes(q);
        }),
      }))
      .filter((group) => (group.items?.length || 0) > 0);
  }, [tags, searchQuery, currentSite, t]);

  return (
    <Drawer
      side="left"
      title={
        <div className="flex items-center gap-2">
          <LogoIcon className="size-4" />
          <span className="text-base font-semibold tracking-wider text-primary flex items-center gap-1.5">
            <span>AVDL</span>
            <button
              type="button"
              onClick={() => hasUpdate && openUpdateDialog()}
              className={`relative inline-flex items-center text-xs font-normal transition-colors ${
                hasUpdate
                  ? 'text-primary cursor-pointer hover:underline'
                  : 'text-muted cursor-default'
              }`}
              title={hasUpdate ? t('update.dialogTitle') : undefined}
            >
              <span>v{pkg.version}</span>
              {hasUpdate && (
                <span className="absolute -top-0.5 -right-2 size-1.5 rounded-full bg-rose-500 shadow-[0_0_6px_rgba(244,63,94,0.8)]" />
              )}
            </button>
          </span>
        </div>
      }
      subtitle={t('siteNav.subtitle')}
      onClose={onClose}
      width="320px"
      bodyClassName="px-2.5 pb-5 pt-1 space-y-5 select-none"
    >
      {({ close }) => {
        const select = (callback: () => void) => {
          callback();
          close();
        };

        const siteList = manifests.length > 0 ? manifests : activeManifest ? [activeManifest] : [];

        return (
          <>
            {/* 1. Jump to Tag 搜索条 */}
            <div className="relative flex items-center">
              <Search
                size={13}
                className="w-3.5 h-3.5 absolute left-2.5 text-muted pointer-events-none"
              />
              <input
                ref={searchInputRef}
                type="text"
                placeholder={t('siteNav.jumpToTag')}
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                className="input-field rounded-md pl-8 pr-12 py-1.5 text-sm"
              />
              {searchQuery ? (
                <button
                  type="button"
                  onClick={() => setSearchQuery('')}
                  className="absolute right-2.5 text-muted hover:text-primary transition-colors cursor-pointer"
                  title={t('siteNav.clearSearch')}
                >
                  <X size={12} />
                </button>
              ) : (
                <kbd className="kbd-shortcut absolute right-2 pointer-events-none gap-0.5">
                  <Command size={9} />K
                </kbd>
              )}
            </div>

            {/* 2. 站点切换器：扁平分段（Segmented），白色选中指示点 */}
            {siteList.length > 0 && (
              <div>
                <div
                  className="grid gap-1 p-1 bg-black/40 border border-default rounded-lg"
                  style={{ gridTemplateColumns: `repeat(${siteList.length}, minmax(0, 1fr))` }}
                >
                  {siteList.map((manifest) => {
                    const isSelected = currentSite === manifest.site;
                    const rawDomain = manifest.primary_domain || '';
                    const domain = rawDomain.replace(/^https?:\/\//, '').replace(/\/$/, '');
                    return (
                      <button
                        key={manifest.site}
                        onClick={() => {
                          const initialUrl = manifest.default_url || manifest.quick_links?.[0]?.url || '';
                          handleSelectSite(manifest.site, initialUrl);
                        }}
                        className={`flex h-14 flex-col items-center justify-center gap-0.5 px-1.5 rounded-md transition-all duration-200 relative cursor-pointer ${
                          isSelected
                            ? 'text-primary bg-white/[0.08] shadow-[0_1px_4px_rgba(0,0,0,0.4)]'
                            : 'text-muted hover:text-secondary hover:bg-white/[0.02]'
                        }`}
                      >
                        <span className="text-sm font-medium tracking-wide">
                          {manifest.name || manifest.site}
                        </span>
                        <span className="text-2xs text-muted truncate max-w-full">{domain}</span>
                        {isSelected && <span className="indicator-dot absolute bottom-1.5" />}
                      </button>
                    );
                  })}
                </div>
              </div>
            )}

            {/* 3. 站点未选择时提示，选中后展示对应站点的分类和标签 */}
            {!currentSite ? (
              <div className="py-12 flex flex-col items-center justify-center text-center text-muted gap-2">
                <span className="text-xs">{t('siteNav.selectSiteToBrowse')}</span>
              </div>
            ) : (
              <>
                {/* HIGHLIGHTS (快捷入口) */}
                {quickLinks.length > 0 && (
                  <div>
                    <div className="flex items-center space-x-1.5 text-section-label mb-2">
                      <Flame size={12} className="text-muted" />
                      <span>{t('siteNav.highlights')}</span>
                    </div>
                    <div className="flex flex-wrap gap-x-3 gap-y-1.5 text-sm">
                      {quickLinks.map((link) => (
                        <button
                          key={link.url}
                          onClick={() => select(() => handleSelectNavItem(link))}
                          className="group relative text-secondary hover:text-primary transition-colors duration-150 py-0.5 text-left cursor-pointer"
                        >
                          <span>{t(`${currentSite}.quickLinks.${link.name}`, link.name)}</span>
                          <span className="absolute left-0 bottom-0 w-0 h-[1px] bg-white/40 transition-all duration-200 group-hover:w-full" />
                        </button>
                      ))}
                    </div>
                  </div>
                )}

                {/* CATEGORIES (分类专区) */}
                {categories.length > 0 && (
                  <div>
                    <div className="flex items-center space-x-1.5 text-section-label mb-2">
                      <Layers size={12} className="text-muted" />
                      <span>{t('siteNav.categories')}</span>
                    </div>
                    <div className="flex flex-wrap gap-x-3 gap-y-1.5 text-sm">
                      {categories.map((category) => (
                        <button
                          key={category.url}
                          onClick={() => select(() => handleSelectNavItem(category))}
                          className="text-muted hover:text-primary transition-all duration-150 text-left cursor-pointer py-0.5"
                        >
                          #{t(`${currentSite}.categories.${category.name}`, category.name)}
                        </button>
                      ))}
                    </div>
                  </div>
                )}

                {/* TAXONOMY INDEX (热门标签分组) */}
                <div className="space-y-4 pt-1">
                  <div className="text-section-label flex items-center justify-between border-b border-default pb-2">
                    <div className="flex items-center space-x-1.5">
                      <Tag size={12} className="text-muted" />
                      <span>{t('siteNav.taxonomyIndex')}</span>
                    </div>
                    <span className="text-meta-sub">{t('siteNav.totalTags', { count: totalTagsCount })}</span>
                  </div>

                  {filteredTagGroups.length === 0 ? (
                    <div className="py-6 text-center text-meta">
                      {searchQuery ? t('siteNav.noMatchQuery', { query: searchQuery }) : t('siteNav.noTagsData')}
                    </div>
                  ) : (
                    filteredTagGroups.map((tag, index) => {
                      const groupKey = `${currentSite}_${tag.group}`;
                      const isSearching = !!searchQuery.trim();
                      const isCollapsed = !isSearching && !!collapsedTagGroups[groupKey];

                      return (
                        <CollapsibleTagGroup
                          key={groupKey}
                          currentSite={currentSite}
                          code={String(index + 1).padStart(2, '0')}
                          tag={tag}
                          collapsed={isCollapsed}
                          onToggle={() => toggleTagGroupCollapsed(groupKey)}
                          onSelect={(item) => select(() => handleSelectNavItem(item))}
                        />
                      );
                    })
                  )}
                </div>
              </>
            )}
          </>
        );
      }}
    </Drawer>
  );
}

function CollapsibleTagGroup({
  currentSite,
  code,
  tag,
  collapsed,
  onToggle,
  onSelect,
}: {
  currentSite: string;
  code: string;
  tag: TagGroup;
  collapsed: boolean;
  onToggle: () => void;
  onSelect: (item: NavItem) => void;
}) {
  const { t } = useTranslation();
  const items = tag?.items || [];
  return (
    <div className="space-y-1.5">
      {/* 微型极客分界线与标签头，整行点击可折叠 */}
      <button
        type="button"
        onClick={onToggle}
        className="w-full flex items-baseline justify-between text-xs text-muted group hover:text-primary transition-colors cursor-pointer select-none py-0.5"
        title={collapsed ? t('siteNav.expandTagGroup') : t('siteNav.collapseTagGroup')}
      >
        <span className="text-meta-sub mr-1.5 group-hover:text-muted">[{code}]</span>
        <span className="font-semibold tracking-wider text-secondary group-hover:text-primary uppercase">
          {t(`${currentSite}.tagGroups.${tag?.group}`, tag?.group || 'TAGS')}
        </span>
        <div className="flex-1 border-b border-dotted border-white/10 group-hover:border-white/25 mx-2" />
        <span className="text-meta-sub group-hover:text-muted">{items.length}</span>
        <ChevronDown
          size={10}
          className={`ml-1.5 transition-transform duration-200 text-muted group-hover:text-secondary ${
            collapsed ? '-rotate-90' : 'rotate-0'
          }`}
        />
      </button>

      {/* 优雅的字符级排列：高密度标签字云 */}
      {!collapsed && (
        <div className="flex flex-wrap gap-x-2 gap-y-1.5 text-sm leading-relaxed">
          {items.map((item) => (
            <button
              key={item.url}
              onClick={() => onSelect(item)}
              className="px-2 py-0.5 rounded text-secondary hover:text-primary hover:bg-white/[0.05] transition-all duration-150 cursor-pointer text-left"
            >
              {t(`${currentSite}.tags.${item.name}`, item.name)}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
