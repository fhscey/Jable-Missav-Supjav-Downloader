import React from 'react';
import { cva, type VariantProps } from 'class-variance-authority';
import { cn } from '../../utils/cn';

export const tagVariants = cva(
  'inline-flex items-center transition-all duration-150 select-none text-xs rounded-md',
  {
    variants: {
      variant: {
        default:
          'border border-default bg-white/5 text-secondary hover:border-medium hover:bg-white/10 hover:text-primary active:scale-95 cursor-pointer',
        ghost: 'text-muted hover:bg-white/5 hover:text-primary active:scale-95 cursor-pointer',
        active: 'border border-active bg-white/15 text-primary font-medium shadow-sm',
        static: 'border border-subtle bg-white/[0.03] text-muted cursor-default',
      },
      size: {
        sm: 'px-1.5 py-0.5 text-xs',
        md: 'px-2 py-0.5 text-sm',
      },
    },
    defaultVariants: {
      variant: 'default',
      size: 'md',
    },
  }
);

export interface TagProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement>, VariantProps<typeof tagVariants> {
  prefixSymbol?: string;
  count?: number | string;
  children: React.ReactNode;
}

export function Tag({
  className,
  variant,
  size,
  prefixSymbol,
  count,
  children,
  type = 'button',
  ...props
}: TagProps) {
  return (
    <button type={type} className={cn(tagVariants({ variant, size }), className)} {...props}>
      {prefixSymbol && <span className="mr-0.5 opacity-60">{prefixSymbol}</span>}
      <span>{children}</span>
      {count !== undefined && <span className="ml-1 text-meta-sub opacity-50">({count})</span>}
    </button>
  );
}
