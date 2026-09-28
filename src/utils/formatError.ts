import i18n from '../i18n';
import { Language } from '../types';

/**
 * 将后端 ExtractorError 或网络异常统一格式化为用户可读的提示文本
 */
export function formatExtractorError(err: unknown, currentSiteName?: string): string {
  if (!err) return i18n.t('errors.unknown');

  if (typeof err === 'object' && err !== null) {
    const errorObj = err as { type?: string; data?: any; message?: string };

    if (errorObj.type === 'UnsupportedLanguage') {
      const lang = errorObj.data as Language | string;
      const langMap: Record<string, string> = {
        'zh-CN': '简体中文 (zh-CN)',
        'zh-TW': '繁體中文 (zh-TW)',
        'ja': '日本語 (ja)',
        'en': 'English (en)',
      };
      const langLabel = langMap[lang] || lang || i18n.t('common.status.unknown');
      const site = currentSiteName || i18n.t('common.status.unknown');
      return i18n.t('errors.unsupportedLanguage', { site, lang: langLabel });
    }

    if (errorObj.type === 'UnsupportedUrl') {
      return i18n.t('errors.unsupportedUrl', { url: errorObj.data || '' });
    }

    if (errorObj.type === 'UnsupportedSite') {
      return i18n.t('errors.unsupportedSite', { site: errorObj.data || '' });
    }

    if (errorObj.type === 'CloudflareBlocked') {
      return i18n.t('errors.cloudflareBlocked');
    }

    if (errorObj.type === 'HtmlGetFailed') {
      return i18n.t('errors.htmlGetFailed');
    }

    if (errorObj.type === 'HtmlParse') {
      return i18n.t('errors.htmlParse');
    }

    if (errorObj.type === 'MediaNotFound') {
      return i18n.t('errors.mediaNotFound');
    }

    if (errorObj.type === 'Http') {
      return i18n.t('errors.httpError', { error: errorObj.data || '' });
    }

    if (errorObj.type === 'Other' && typeof errorObj.data === 'string') {
      const lower = errorObj.data.toLowerCase();
      if (lower.includes('timeout') || lower.includes('超时') || lower.includes('timed out')) {
        return i18n.t('errors.timeout');
      }
      return errorObj.data;
    }

    if (errorObj.message) {
      return errorObj.message;
    }
  }

  const str = String(err);
  if (str.includes('UnsupportedLanguage')) {
    const site = currentSiteName || i18n.t('common.status.unknown');
    if (str.includes('ZHCN') || str.includes('zh-CN')) {
      return i18n.t('errors.unsupportedLanguage', { site, lang: '简体中文 (zh-CN)' });
    }
    return i18n.t('errors.unsupportedLanguage', { site, lang: str });
  }
  if (str.includes('timeout') || str.includes('超时')) {
    return i18n.t('errors.timeout');
  }

  return str;
}
