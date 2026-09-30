import React, { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { LoaderCircle, Search, X } from 'lucide-react';
import { useSiteStore } from '../../store/siteStore';
import { useUIStore } from '../../store/uiStore';

interface SearchInputProps {
  isFocused?: boolean;
  onFocusChange: (focused: boolean) => void;
  onAutoWakeup?: () => void;
  onExit?: () => void;
}

export function SearchInput({
  isFocused = false,
  onFocusChange,
  onAutoWakeup,
  onExit,
}: SearchInputProps) {
  const { t } = useTranslation();
  const searchKeyword = useSiteStore((s) => s.searchKeyword);
  const handleSearchSubmit = useSiteStore((s) => s.handleSearchSubmit);
  const exitSearch = useSiteStore((s) => s.exitSearch);
  const isVideoLoading = useUIStore((s) => s.isVideoLoading);
  const inputRef = useRef<HTMLInputElement>(null);

  const isSearching = Boolean(isVideoLoading && searchKeyword);

  // 输入框草稿：纯组件内部局部状态，打字不触发全局 store 与 query 变更
  const [draft, setDraft] = useState(searchKeyword);

  // 外部全局搜索词变化（如切换站点、点击分类或退出搜索时）同步重置本地草稿
  useEffect(() => {
    setDraft(searchKeyword);
  }, [searchKeyword]);

  const handleExit = () => {
    setDraft('');
    inputRef.current?.blur();
    onFocusChange(false);
    exitSearch();
    onExit?.();
  };

  // 快捷键 '/' 唤起搜索；'Escape' 退出搜索
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      // 1. 按 '/' 快速唤出并聚焦搜索框（仅在未聚焦输入元素时触发）
      if (
        event.key === '/' &&
        !(
          event.target instanceof HTMLInputElement ||
          event.target instanceof HTMLTextAreaElement ||
          (event.target as HTMLElement)?.isContentEditable
        )
      ) {
        event.preventDefault();
        if (onAutoWakeup) onAutoWakeup();
        setTimeout(() => {
          inputRef.current?.focus();
          inputRef.current?.select();
        }, 30);
        return;
      }

      // 2. 按 'Escape' 退出搜索（在搜索模式或输入框聚焦时生效，且不与上层弹窗冲突）
      if (event.key === 'Escape') {
        const { activeDrawer, selectedCard, playingMedia } = useUIStore.getState();
        const hasOverlay =
          activeDrawer !== 'none' || Boolean(selectedCard) || Boolean(playingMedia);
        if (hasOverlay) return;

        if (document.activeElement === inputRef.current || draft) {
          event.preventDefault();
          handleExit();
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [onAutoWakeup, draft, onFocusChange, exitSearch]);

  const handleInputKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      handleExit();
    }
  };

  const onSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    handleSearchSubmit(draft);
    inputRef.current?.blur();
    onFocusChange(false);
  };

  return (
    <form
      className={`pointer-events-auto absolute left-1/2 -translate-x-1/2 flex h-7 w-[min(28rem,calc(100vw-22rem))] items-center gap-1.5 rounded-lg border px-2.5 backdrop-blur-md transition-all duration-200 ${
        isFocused
          ? 'bg-surface-hud border-active shadow-hud text-primary'
          : 'bg-surface-control border-default hover:border-medium hover:bg-surface-hover text-muted'
      }`}
      onSubmit={onSubmit}
    >
      {isSearching ? (
        <LoaderCircle size={13} className="animate-spin text-primary shrink-0" />
      ) : (
        <Search size={13} strokeWidth={1.8} className={isFocused ? 'text-primary' : 'text-muted'} />
      )}
      <input
        ref={inputRef}
        className="min-w-0 flex-1 bg-transparent text-sm text-primary outline-none placeholder:text-muted"
        placeholder={t('search.placeholder')}
        value={draft}
        onFocus={() => onFocusChange(true)}
        onBlur={() => onFocusChange(false)}
        onKeyDown={handleInputKeyDown}
        onChange={(event) => setDraft(event.target.value)}
      />

      {draft && (
        <button
          type="button"
          onClick={handleExit}
          className="grid size-4.5 place-items-center rounded text-muted hover:text-primary hover:bg-surface-hover transition-colors cursor-pointer"
          title={t('search.exitSearch')}
          aria-label={t('search.exitSearch')}
        >
          <X size={11} />
        </button>
      )}

      <kbd className="kbd-shortcut hidden sm:inline-flex">{isFocused ? 'ESC' : '/'}</kbd>
    </form>
  );
}
