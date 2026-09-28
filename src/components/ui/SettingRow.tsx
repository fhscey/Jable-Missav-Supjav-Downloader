import React from 'react';
import { cn } from '../../utils/cn';

export interface SettingSectionProps {
  title: string;
  children: React.ReactNode;
  className?: string;
}

export function SettingSection({ title, children, className }: SettingSectionProps) {
  return (
    <section className={className}>
      <div className="mb-1 flex items-center justify-between px-1 text-2xs font-semibold tracking-wider text-white/35 uppercase">
        <span>{title}</span>
      </div>
      <div className="rounded-lg border border-white/[0.08] bg-white/[0.02] divide-y divide-white/[0.05]">
        {children}
      </div>
    </section>
  );
}

export interface SettingRowProps {
  label: React.ReactNode;
  leading?: React.ReactNode;
  subtitle?: React.ReactNode;
  tip?: string;
  children?: React.ReactNode;
  className?: string;
}

export function SettingRow({
  label,
  leading,
  subtitle,
  tip,
  children,
  className,
}: SettingRowProps) {
  return (
    <div
      className={cn(
        'flex min-h-[34px] items-center justify-between gap-2 px-2.5 py-1 select-none transition-colors duration-150 hover:bg-white/[0.035] first:rounded-t-lg last:rounded-b-lg',
        className
      )}
    >
      <div className="flex min-w-0 flex-1 flex-col pr-1">
        <div className="flex items-center gap-1.5 shrink-0">
          {leading}
          <div className="group relative flex items-center gap-1 shrink-0 hover:z-50">
            <span
              className={cn(
                'text-xs text-white/70 font-normal transition-colors',
                tip && 'hover:text-white cursor-help'
              )}
            >
              {label}
            </span>
            {tip && (
              <div className="pointer-events-none absolute bottom-full left-0 z-50 mb-2 w-max max-w-[17rem] scale-95 opacity-0 transition-all duration-150 group-hover:scale-100 group-hover:opacity-100">
                <div className="rounded-lg border border-white/10 bg-zinc-950/95 px-2.5 py-1.5 shadow-2xl shadow-black/80 backdrop-blur-md">
                  <span className="block text-2xs leading-relaxed text-zinc-300">{tip}</span>
                </div>
                <div className="absolute -bottom-1 left-4 size-2 rotate-45 border-b border-r border-white/10 bg-zinc-950/95" />
              </div>
            )}
          </div>
        </div>
        {subtitle && (
          <div className="group/sub relative min-w-0 mt-0.5">
            <span
              className="block truncate text-2xs text-white/35 font-mono select-all hover:text-white/60 transition-colors cursor-default"
              title={typeof subtitle === 'string' ? subtitle : undefined}
            >
              {subtitle}
            </span>
            <div className="pointer-events-none absolute bottom-full left-0 z-50 mb-1.5 w-max max-w-[22rem] scale-95 opacity-0 transition-all duration-150 group-hover/sub:scale-100 group-hover/sub:opacity-100 group-hover/sub:pointer-events-auto">
              <div className="rounded-lg border border-white/10 bg-zinc-950/95 px-2.5 py-1.5 shadow-2xl shadow-black/80 backdrop-blur-md">
                <span className="block break-all text-2xs leading-relaxed text-zinc-200 select-all font-mono">
                  {subtitle}
                </span>
              </div>
              <div className="absolute -bottom-1 left-4 size-2 rotate-45 border-b border-r border-white/10 bg-zinc-950/95" />
            </div>
          </div>
        )}
      </div>
      {children && <div className="flex shrink-0 items-center justify-end">{children}</div>}
    </div>
  );
}
