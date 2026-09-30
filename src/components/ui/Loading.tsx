import React, { useEffect, useState } from 'react';
import { LoaderCircle } from 'lucide-react';
import { useTranslation } from 'react-i18next';

export interface LoadingProps {
  message?: string;
  className?: string;
}

const DOTS_CYCLE = ['.', '..', '...'];

export const Loading: React.FC<LoadingProps> = ({ message, className = '' }) => {
  const { t } = useTranslation();
  const [dotIndex, setDotIndex] = useState(0);

  useEffect(() => {
    const timer = setInterval(() => {
      setDotIndex((prev) => (prev + 1) % DOTS_CYCLE.length);
    }, 450);
    return () => clearInterval(timer);
  }, []);

  // 提取主体文案，剥离末尾原有的省略号/句号以配合动态点阵
  const rawText = message || t('common.status.loading', '加载中...');
  const baseText = rawText.replace(/[\.。…]+$/, '');

  return (
    <div
      className={`pointer-events-none absolute inset-0 z-30 flex flex-col items-center justify-center select-none animate-in fade-in duration-200 ${className}`}
    >
      <div className="flex flex-col items-center">
        <LoaderCircle className="animate-spin text-white/80 mb-2.5" size={28} strokeWidth={1.8} />
        <p className="text-xs font-medium text-white/70 tracking-wider flex items-center">
          <span>{baseText}</span>
          <span className="inline-block w-4 text-left font-mono">{DOTS_CYCLE[dotIndex]}</span>
        </p>
      </div>
    </div>
  );
};

export default Loading;
