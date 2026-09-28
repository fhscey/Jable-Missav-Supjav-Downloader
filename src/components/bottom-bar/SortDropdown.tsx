import { useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ListFilter } from 'lucide-react';
import { useSiteStore } from '../../store/siteStore';
import { useCurrentSortRule } from '../../hooks/queries';
import { barItemVariants } from './BarItem';
import { useClickOutside } from '../../hooks/useClickOutside';

interface SortDropdownProps {
  onMenuOpenChange?: (open: boolean) => void;
}

export function SortDropdown({ onMenuOpenChange }: SortDropdownProps) {
  const { t } = useTranslation();
  const currentSort = useSiteStore((s) => s.currentSort);
  const handleSortSelect = useSiteStore((s) => s.handleSortSelect);
  const sortRule = useCurrentSortRule();
  const [sortMenuOpen, setSortMenuOpen] = useState(false);
  const sortMenuRef = useRef<HTMLDivElement>(null);
  const hasSortOptions = Boolean(sortRule?.options.length);

  const updateMenuOpen = (open: boolean | ((prev: boolean) => boolean)) => {
    setSortMenuOpen((prev) => {
      const next = typeof open === 'function' ? open(prev) : open;
      if (onMenuOpenChange) onMenuOpenChange(next);
      return next;
    });
  };

  useClickOutside(sortMenuRef, () => updateMenuOpen(false), {
    enabled: sortMenuOpen,
  });

  return (
    <div ref={sortMenuRef} className="relative">
      <button
        type="button"
        className={barItemVariants({ active: sortMenuOpen, disabled: !hasSortOptions })}
        onClick={() => hasSortOptions && updateMenuOpen((open) => !open)}
        title={t('bottomBar.sort')}
        disabled={!hasSortOptions}
      >
        <ListFilter size={14} strokeWidth={1.8} />
      </button>

      {sortMenuOpen && sortRule && (
        <div className="glass-surface absolute right-0 bottom-[calc(100%+10px)] min-w-32 rounded-xl p-1 animate-in fade-in zoom-in-95 duration-100 select-none space-y-0.5">
          {sortRule.options.map((option) => {
            const selected = currentSort === option.value;
            const label =
              t(`sort.${option.value}`, { defaultValue: '' }) ||
              t(`sort.${option.name}`, { defaultValue: '' }) ||
              option.name;
            return (
              <button
                key={option.value}
                type="button"
                onClick={() => {
                  handleSortSelect(option.value);
                  updateMenuOpen(false);
                }}
                className={`flex w-full items-center justify-between rounded-md px-2.5 py-1.5 text-left text-body-sm transition cursor-pointer ${
                  selected
                    ? 'bg-white/10 font-medium text-primary'
                    : 'text-muted hover:bg-white/5 hover:text-primary'
                }`}
              >
                <span>{label}</span>
                {selected && <span className="indicator-dot ml-2 shrink-0" />}
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
