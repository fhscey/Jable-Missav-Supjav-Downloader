import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import type { Language } from '../types';

import en from './locales/en.json';
import ja from './locales/ja.json';
import zhCN from './locales/zh-CN.json';
import zhTW from './locales/zh-TW.json';

// 1. 语言显示名称（未来下拉菜单/设置页也可以直接复用）
export const LANGUAGE_LABELS: Record<Language, string> = {
  'zh-TW': '繁體中文',
  'zh-CN': '简体中文',
  'en': 'English',
  'ja': '日本語',
};

// 2. 强类型约束 resources 必须覆盖所有 Language
export const resources: Record<Language, { translation: Record<string, any> }> = {
  'en': { translation: en },
  'ja': { translation: ja },
  'zh-CN': { translation: zhCN },
  'zh-TW': { translation: zhTW },
};

i18n
  .use(initReactI18next)
  .init({
    resources,
    lng: 'zh-CN',
    fallbackLng: 'en', // 如果 ja/zh 没填内容，自动回退到 en
    interpolation: {
      escapeValue: false,
    },
  });

export default i18n;
