import React from 'react';
import { cva, type VariantProps } from 'class-variance-authority';
import { cn } from '../../utils/cn';

export const barItemVariants = cva(
  'btn-control',
  {
    variants: {
      active: {
        true: 'border border-white/20 bg-white/15 text-white',
        false: '',
      },
      disabled: {
        true: 'opacity-30',
        false: '',
      },
    },
    defaultVariants: {
      active: false,
      disabled: false,
    },
  }
);

export interface BarItemProps extends VariantProps<typeof barItemVariants> {
  isActive?: boolean;
  disabled?: boolean;
  title: string;
  onClick: (e: React.MouseEvent<HTMLButtonElement>) => void;
  badge?: React.ReactNode;
  children: React.ReactNode;
  className?: string;
}

export function BarItem({
  isActive = false,
  disabled = false,
  title,
  onClick,
  badge,
  children,
  className,
}: BarItemProps) {
  return (
    <button
      type="button"
      className={cn(barItemVariants({ active: isActive, disabled }), className)}
      onClick={onClick}
      disabled={disabled}
      title={title}
      aria-label={title}
    >
      {children}
      {badge}
    </button>
  );
}
