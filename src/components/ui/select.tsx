import { useRef, useState } from 'react';
import { ChevronDown } from 'lucide-react';
import { cn } from '../../utils/cn';
import { useClickOutside } from '../../hooks/useClickOutside';

export interface SelectOption<T extends string = string> {
  value: T;
  label: string;
}

export interface SelectProps<T extends string = string> {
  value: T;
  options: SelectOption<T>[] | [string, string][];
  onChange: (value: T) => void;
  className?: string;
  triggerClassName?: string;
  disabled?: boolean;
}

export function Select<T extends string = string>({
  value,
  options,
  onChange,
  className,
  triggerClassName,
  disabled = false,
}: SelectProps<T>) {
  const [open, setOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  useClickOutside(containerRef, () => setOpen(false), { enabled: open });

  // 格式化为统一的 { value, label } 结构
  const normalizedOptions: SelectOption<T>[] = options.map((opt) => {
    if (Array.isArray(opt)) {
      return { value: opt[0] as T, label: opt[1] };
    }
    return opt;
  });

  const currentLabel =
    normalizedOptions.find((opt) => opt.value === value)?.label || value;

  return (
    <div ref={containerRef} className={cn('relative select-none', open && 'z-40', className)}>
      <button
        type="button"
        disabled={disabled}
        onClick={(e) => {
          e.stopPropagation();
          setOpen((prev) => !prev);
        }}
        className={cn(
          'flex h-6 min-w-16 items-center justify-between gap-1 rounded px-1.5 text-xs font-medium text-white/85 transition hover:bg-white/10 hover:text-white cursor-pointer disabled:pointer-events-none disabled:opacity-40',
          triggerClassName
        )}
      >
        <span className="truncate">{currentLabel}</span>
        <ChevronDown
          size={10}
          className={cn(
            'transition-transform duration-200 text-white/35',
            open && 'rotate-180 text-white'
          )}
        />
      </button>

      {open && (
        <div
          className="glass-surface absolute right-0 top-[calc(100%+4px)] z-50 min-w-28 rounded-lg p-1 animate-in fade-in zoom-in-95 duration-100"
          onClick={(e) => e.stopPropagation()}
        >
          {normalizedOptions.map((opt) => {
            const isSelected = opt.value === value;
            return (
              <button
                key={opt.value}
                type="button"
                className={cn(
                  'flex w-full items-center justify-between rounded px-2 py-1 text-left text-xs transition cursor-pointer whitespace-nowrap',
                  isSelected
                    ? 'font-medium text-white bg-white/10'
                    : 'text-white/60 hover:bg-white/5 hover:text-white'
                )}
                onClick={(e) => {
                  e.stopPropagation();
                  onChange(opt.value);
                  setOpen(false);
                }}
              >
                <span>{opt.label}</span>
                {isSelected && <span className="indicator-dot ml-2 shrink-0" />}
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}

export default Select;
