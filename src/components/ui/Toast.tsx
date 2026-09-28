import React from 'react';
import { useTranslation } from 'react-i18next';
import { X } from 'lucide-react';
import { useUIStore, ToastType } from '../../store/uiStore';

const toastMeta: Record<ToastType, { code: string; dotClass: string }> = {
  success: {
    code: 'OK',
    dotClass: 'bg-white shadow-[0_0_6px_rgba(255,255,255,0.9)]',
  },
  error: {
    code: 'ERR',
    dotClass: 'bg-rose-400 shadow-[0_0_6px_rgba(244,63,94,0.7)]',
  },
  warning: {
    code: 'WARN',
    dotClass: 'bg-amber-300 shadow-[0_0_6px_rgba(252,211,77,0.7)]',
  },
  info: {
    code: 'INFO',
    dotClass: 'bg-white/50',
  },
};

export const Toast: React.FC = () => {
  const { t } = useTranslation();
  const toasts = useUIStore((s) => s.toasts);
  const dismissToast = useUIStore((s) => s.dismissToast);

  if (!toasts || toasts.length === 0) return null;

  return (
    <div className="fixed top-6 left-1/2 -translate-x-1/2 z-[9999] flex flex-col items-center gap-1.5 pointer-events-none select-none">
      {toasts.map((toast) => {
        const meta = toastMeta[toast.type] || toastMeta.info;

        return (
          <div
            key={toast.id}
            className="glass-surface pointer-events-auto flex items-center gap-2.5 px-3 py-1.5 rounded-xl transition-all duration-200 animate-in fade-in slide-in-from-top-2 max-w-[460px]"
          >
            <div className="flex shrink-0 items-center gap-1.5">
              <span className={`size-1.5 rounded-full ${meta.dotClass}`} />
              <span className="text-section-label">{meta.code}</span>
            </div>

            <span className="h-3 w-[1px] bg-white/10 shrink-0" />

            <span className="flex-1 text-sm text-primary leading-snug break-words">
              {toast.message}
            </span>

            <button
              type="button"
              onClick={() => dismissToast(toast.id)}
              className="grid size-5 shrink-0 place-items-center rounded text-muted hover:text-primary hover:bg-white/[0.08] transition cursor-pointer"
              title={t('common.action.close')}
              aria-label={t('common.action.close')}
            >
              <X size={11} />
            </button>
          </div>
        );
      })}
    </div>
  );
};
