import React from 'react';
import { cva, type VariantProps } from 'class-variance-authority';
import { cn } from '../../utils/cn';

export const buttonVariants = cva(
  'inline-flex items-center justify-center font-medium transition-all duration-150 select-none cursor-pointer focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-white/30 disabled:pointer-events-none disabled:opacity-30',
  {
    variants: {
      variant: {
        primary:
          'border border-active bg-white/15 text-primary hover:bg-white/25 active:scale-95 shadow-sm',
        secondary:
          'border border-default bg-white/5 text-secondary hover:border-medium hover:bg-white/10 hover:text-primary active:scale-95',
        ghost:
          'text-muted hover:bg-white/10 hover:text-primary active:scale-90',
        control:
          'btn-control',
        danger:
          'border border-rose-500/30 bg-rose-500/20 text-rose-200 hover:bg-rose-500/30 active:scale-95',
      },
      size: {
        xs: 'h-6 px-2 text-xs rounded gap-1',
        sm: 'h-6.5 px-2.5 text-sm rounded-md gap-1',
        md: 'h-7 px-3 text-base rounded-lg gap-1.5',
        lg: 'h-8 px-4 text-base rounded-lg gap-2',
        icon: 'size-7 p-0 rounded-lg',
        'icon-xs': 'size-5 p-0 rounded',
        'icon-sm': 'size-6 p-0 rounded-md',
      },
    },
    defaultVariants: {
      variant: 'secondary',
      size: 'sm',
    },
  }
);

export interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {
  children: React.ReactNode;
}

export function Button({
  className,
  variant,
  size,
  children,
  type = 'button',
  ...props
}: ButtonProps) {
  return (
    <button
      type={type}
      className={cn(buttonVariants({ variant, size }), className)}
      {...props}
    >
      {children}
    </button>
  );
}

export default Button;
