import { Check, Minus } from 'lucide-react';
import { cn } from '../../utils/cn';

export interface CheckboxProps {
  checked: boolean;
  indeterminate?: boolean;
  onChange?: (checked: boolean) => void;
  disabled?: boolean;
  className?: string;
  id?: string;
  'aria-label'?: string;
}

export function Checkbox({
  checked,
  indeterminate = false,
  onChange,
  disabled = false,
  className,
  id,
  'aria-label': ariaLabel,
}: CheckboxProps) {
  return (
    <label
      className={cn(
        'relative inline-flex items-center justify-center select-none cursor-pointer shrink-0',
        disabled && 'pointer-events-none opacity-40',
        className
      )}
      onClick={(e) => e.stopPropagation()}
    >
      <input
        id={id}
        type="checkbox"
        className="peer sr-only"
        checked={checked}
        disabled={disabled}
        onChange={(e) => onChange?.(e.target.checked)}
        aria-label={ariaLabel}
      />
      <span
        className={cn(
          'flex size-3.5 items-center justify-center rounded-[3px] border transition-all duration-150',
          indeterminate
            ? 'border-white/60 bg-white/15 text-white'
            : checked
              ? 'border-white bg-white text-zinc-950 shadow-[0_0_8px_rgba(255,255,255,0.25)]'
              : 'border-white/20 bg-white/[0.04] hover:border-white/40 hover:bg-white/[0.08]'
        )}
      >
        {indeterminate ? (
          <Minus size={9} strokeWidth={3} />
        ) : checked ? (
          <Check size={9} strokeWidth={3.5} />
        ) : null}
      </span>
    </label>
  );
}
