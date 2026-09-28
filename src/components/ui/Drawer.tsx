import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { cva } from 'class-variance-authority';
import { X } from 'lucide-react';
import { cn } from '../../utils/cn';

export const DrawerContext = createContext<{ close: () => void }>({ close: () => {} });
export const useDrawer = () => useContext(DrawerContext);

type DrawerSide = 'left' | 'right';

const backdropVariants = cva(
  'fixed inset-0 z-[189] bg-black/45 backdrop-blur-[2px] transition-opacity',
  {
    variants: {
      isClosing: {
        true: 'animate-[drawer-backdrop-out_180ms_ease-in_forwards]',
        false: 'animate-[drawer-backdrop-in_200ms_ease-out]',
      },
    },
    defaultVariants: {
      isClosing: false,
    },
  }
);

const drawerShellVariants = cva(
  'drawer-shell',
  {
    variants: {
      side: {
        left: 'left-0',
        right: 'right-0',
      },
      isClosing: {
        true: '',
        false: '',
      },
    },
    compoundVariants: [
      {
        side: 'left',
        isClosing: false,
        className: 'animate-[drawer-in-left_220ms_cubic-bezier(.2,.8,.2,1)]',
      },
      {
        side: 'left',
        isClosing: true,
        className: 'animate-[drawer-out-left_180ms_cubic-bezier(.2,.8,.2,1)_forwards]',
      },
      {
        side: 'right',
        isClosing: false,
        className: 'animate-[drawer-in-right_220ms_cubic-bezier(.2,.8,.2,1)]',
      },
      {
        side: 'right',
        isClosing: true,
        className: 'animate-[drawer-out-right_180ms_cubic-bezier(.2,.8,.2,1)_forwards]',
      },
    ],
    defaultVariants: {
      side: 'left',
      isClosing: false,
    },
  }
);

interface DrawerRenderHelpers {
  close: () => void;
}

interface DrawerProps {
  side: DrawerSide;
  title: ReactNode;
  subtitle?: ReactNode;
  onClose: () => void;
  children: ReactNode | ((helpers: DrawerRenderHelpers) => ReactNode);
  width?: string;
  bodyClassName?: string;
  headerExtra?: ReactNode;
}

// Overlay drawer module: entry/exit motion, Escape close, outside backdrop click, wheel-safe scrolling.
export function Drawer({
  side,
  title,
  subtitle,
  onClose,
  children,
  width = '280px',
  bodyClassName = '',
  headerExtra,
}: DrawerProps) {
  const { t } = useTranslation();
  const [isClosing, setIsClosing] = useState(false);

  const handleClose = useCallback(() => {
    if (isClosing) return;
    setIsClosing(true);
    setTimeout(() => {
      onClose();
    }, 180);
  }, [isClosing, onClose]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        handleClose();
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [handleClose]);

  return (
    <DrawerContext.Provider value={{ close: handleClose }}>
      {/* 遮罩背景：点击外部区域关闭 Drawer */}
      <div
        className={cn(backdropVariants({ isClosing }))}
        onClick={handleClose}
        aria-hidden="true"
      />

      {/* 抽屉主体外壳 */}
      <aside
        className={cn(drawerShellVariants({ side, isClosing }))}
        style={{ width, maxWidth: 'calc(100vw - 2rem)' }}
        role="dialog"
        aria-modal="true"
      >
        <header className="flex shrink-0 items-center justify-between px-3 py-2 border-b border-white/[0.06] mb-1">
          <div className="flex flex-col gap-0.5 min-w-0">
            <div className="text-sm font-semibold text-white/90 tracking-wide">
              {title}
            </div>
            {subtitle && (
              <div className="text-2xs text-white/35 tracking-normal">
                {subtitle}
              </div>
            )}
          </div>
          <div className="flex items-center gap-1 shrink-0">
            {headerExtra}
            <button
              className="btn-control !size-5.5 text-white/40 hover:text-white"
              onClick={handleClose}
              title={t('common.action.close')}
              aria-label={t('common.action.close')}
            >
              <X size={12} />
            </button>
          </div>
        </header>
        <div
          data-overlay-scroll
          className={cn('h-0 flex-1 overflow-y-auto overscroll-contain', bodyClassName)}
        >
          {typeof children === 'function' ? children({ close: handleClose }) : children}
        </div>
      </aside>
    </DrawerContext.Provider>
  );
}
