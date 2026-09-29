import { cn } from '../../utils/cn';

export interface ToggleProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
  className?: string;
  id?: string;
  'aria-label'?: string;
}

export function Toggle({
  checked,
  onChange,
  disabled = false,
  className,
  id,
  'aria-label': ariaLabel,
}: ToggleProps) {
  return (
    <label
      className={cn(
        'relative inline-flex items-center select-none cursor-pointer',
        disabled && 'pointer-events-none opacity-40',
        className
      )}
    >
      <input
        id={id}
        type="checkbox"
        className="peer sr-only"
        checked={checked}
        disabled={disabled}
        onChange={(e) => onChange(e.target.checked)}
        aria-label={ariaLabel}
      />
      <span className="relative block h-4 w-7.5 shrink-0 rounded-full border border-white/10 bg-white/10 transition-colors duration-200 peer-checked:border-white/80 peer-checked:bg-white/80 peer-focus-visible:ring-1 peer-focus-visible:ring-white/40 after:absolute after:left-0.5 after:top-0.5 after:size-2.5 after:rounded-full after:bg-white/60 after:transition-transform after:duration-200 peer-checked:after:translate-x-3.5 peer-checked:after:bg-zinc-950" />
    </label>
  );
}
