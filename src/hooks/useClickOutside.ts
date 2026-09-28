import { useEffect, type RefObject } from 'react';

interface UseClickOutsideOptions {
  enabled?: boolean;
  closeOnEsc?: boolean;
  ignoreSelector?: string;
}

export function useClickOutside(
  ref: RefObject<HTMLElement | null>,
  handler: () => void,
  options: UseClickOutsideOptions = {}
) {
  const { enabled = true, closeOnEsc = true, ignoreSelector } = options;

  useEffect(() => {
    if (!enabled) return;

    const handleMouseDown = (event: MouseEvent) => {
      const target = event.target as Node | null;
      if (!target || !ref.current) return;

      if (ignoreSelector && (event.target as Element).closest?.(ignoreSelector)) {
        return;
      }

      if (!ref.current.contains(target)) {
        handler();
      }
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (closeOnEsc && event.key === 'Escape') {
        handler();
      }
    };

    window.addEventListener('mousedown', handleMouseDown);
    if (closeOnEsc) {
      window.addEventListener('keydown', handleKeyDown);
    }

    return () => {
      window.removeEventListener('mousedown', handleMouseDown);
      if (closeOnEsc) {
        window.removeEventListener('keydown', handleKeyDown);
      }
    };
  }, [ref, handler, enabled, closeOnEsc, ignoreSelector]);
}

export default useClickOutside;
