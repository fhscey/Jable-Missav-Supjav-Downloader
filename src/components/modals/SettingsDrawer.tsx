import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { FolderOpen, FolderSync, RefreshCw, RotateCcw } from 'lucide-react';
import { DeleteFileMode, Language, PreferredQuality } from '../../types';
import { tauriApi } from '../../api';
import { useUIStore } from '../../store/uiStore';
import { useUpdateStore } from '../../store/updateStore';
import {
  getDeleteFileMode,
  setDeleteFileMode,
  useAppConfig,
  useUpdateConfig,
} from '../../hooks/queries';
import { Drawer } from '../ui/Drawer';
import { LogoIcon } from '../ui/AvLogoIcon';
import { Button } from '../ui/Button';
import { Toggle } from '../ui/Toggle';
import { Select } from '../ui/Select';
import { SettingRow, SettingSection } from '../ui/SettingRow';
import { DiskSpaceRing } from '../ui/DiskSpaceRing';

interface SettingsDrawerProps {
  onClose: () => void;
}

function validateProxyUrl(
  val: string,
  t: (key: string) => string
): { valid: boolean; error?: string; normalized?: string } {
  const trimmed = val.trim();
  if (!trimmed) {
    return { valid: false, error: t('settings.items.proxy.emptyError') };
  }
  try {
    const hasScheme = trimmed.includes('://');
    const urlStr = hasScheme ? trimmed : `http://${trimmed}`;
    const parsed = new URL(urlStr);
    if (!parsed.hostname) {
      return { valid: false, error: t('settings.items.proxy.missingHost') };
    }
    const port = parseInt(parsed.port, 10);
    if (!parsed.port || isNaN(port) || port <= 0 || port > 65535) {
      return { valid: false, error: t('settings.items.proxy.invalidPort') };
    }
    const scheme = parsed.protocol.replace(':', '').toLowerCase();
    if (!['http', 'https', 'socks5', 'socks5h'].includes(scheme)) {
      return { valid: false, error: t('settings.items.proxy.unsupportedScheme') };
    }
    return { valid: true, normalized: hasScheme ? trimmed : `http://${trimmed}` };
  } catch {
    return { valid: false, error: t('settings.items.proxy.formatError') };
  }
}

function GameAutoSaveBadge({ triggerId }: { triggerId: number }) {
  const [visible, setVisible] = useState(false);
  const timerRef = useRef<any>(null);
  const initialMountRef = useRef(true);
  const prevTriggerIdRef = useRef<number>(triggerId);

  useEffect(() => {
    if (initialMountRef.current) {
      initialMountRef.current = false;
      prevTriggerIdRef.current = triggerId;
      return;
    }

    if (triggerId > prevTriggerIdRef.current) {
      prevTriggerIdRef.current = triggerId;
      setVisible(true);
      if (timerRef.current) clearTimeout(timerRef.current);
      timerRef.current = setTimeout(() => {
        setVisible(false);
      }, 1200);
    }
  }, [triggerId]);

  if (!visible) return null;

  return (
    <div
      key={triggerId}
      className="animate-[game-save-spin_1.15s_cubic-bezier(0.25,0.1,0.25,1)_forwards] [transform-style:preserve-3d] flex size-4 items-center justify-center select-none shrink-0"
    >
      <LogoIcon className="size-4" />
    </div>
  );
}

function SpeedControl({
  speedBytes,
  onChange,
}: {
  speedBytes: number;
  onChange: (speedBytes: number) => void;
}) {
  const { t } = useTranslation();
  const [editing, setEditing] = useState(false);
  const mbps = speedBytes > 0 ? Math.round(speedBytes / (1024 * 1024)) : 0;
  const [val, setVal] = useState(String(mbps));
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    setVal(String(mbps));
  }, [mbps]);

  useEffect(() => {
    if (editing && inputRef.current) {
      inputRef.current.focus();
      inputRef.current.select();
    }
  }, [editing]);

  const commit = () => {
    setEditing(false);
    const parsed = parseInt(val.trim(), 10);
    const num = isNaN(parsed) || parsed < 0 ? 0 : parsed;
    setVal(String(num));
    if (num * 1024 * 1024 !== speedBytes) {
      onChange(num * 1024 * 1024);
    }
  };

  if (editing) {
    return (
      <div className="flex h-6 items-center gap-1 rounded border border-white/20 bg-black/50 px-1.5 shadow-inner">
        <input
          ref={inputRef}
          type="text"
          inputMode="numeric"
          pattern="[0-9]*"
          className="w-9 bg-transparent text-right text-xs text-white outline-none"
          value={val}
          onChange={(e) => setVal(e.target.value.replace(/[^\d]/g, ''))}
          onBlur={commit}
          onKeyDown={(e) => {
            if (e.key === 'Enter') commit();
            if (e.key === 'Escape') {
              setEditing(false);
              setVal(String(mbps));
            }
          }}
        />
        <span className="text-2xs text-white/40 select-none">MB/s</span>
      </div>
    );
  }

  return (
    <button
      type="button"
      onClick={() => setEditing(true)}
      className="group/spd flex h-6 items-center gap-1.5 rounded px-1.5 text-xs transition hover:bg-white/10 cursor-pointer"
      title={t('settings.items.bandwidthLimit.editTitle')}
    >
      <span
        className={
          mbps === 0 ? 'text-white/60 group-hover/spd:text-white' : 'font-medium text-white/90'
        }
      >
        {mbps === 0 ? t('settings.items.bandwidthLimit.unlimited') : `${mbps} MB/s`}
      </span>
      <span className="text-2xs text-white/30 transition-colors group-hover/spd:text-white/60">
        {t('settings.items.bandwidthLimit.edit')}
      </span>
    </button>
  );
}

export function SettingsDrawer({ onClose }: SettingsDrawerProps) {
  const { t } = useTranslation();
  const { data: config } = useAppConfig();
  const {
    updateConfig,
    selectDir,
    openDownloadDir,
    resetToDefaults,
    saveTriggerId,
  } = useUpdateConfig();

  const [deleteFileMode, setLocalDeleteFileMode] = useState<DeleteFileMode>(getDeleteFileMode());

  const handleSetDeleteFileMode = (mode: DeleteFileMode) => {
    setDeleteFileMode(mode);
    setLocalDeleteFileMode(mode);
  };

  const [proxyInput, setProxyInput] = useState('');
  const [proxyError, setProxyError] = useState('');
  const [isCheckingUpdate, setIsCheckingUpdate] = useState(false);
  const showToast = useUIStore((s) => s.showToast);

  const handleCheckUpdate = async () => {
    if (isCheckingUpdate) return;
    setIsCheckingUpdate(true);
    try {
      const info = await tauriApi.checkForUpdate();
      if (info.updateAvailable) {
        showToast(
          t('settings.items.checkUpdate.newVersion', { version: info.latestVersion }),
          'success'
        );
        useUpdateStore.getState().setUpdateAvailable(info);
      } else {
        showToast(
          t('settings.items.checkUpdate.upToDate', { version: info.currentVersion }),
          'info'
        );
      }
    } catch (err: any) {
      showToast(
        t('settings.items.checkUpdate.failed', {
          error: err?.message || String(err),
        }),
        'error'
      );
    } finally {
      setIsCheckingUpdate(false);
    }
  };


  useEffect(() => {
    if (config?.proxy_mode?.type === 'Custom') {
      setProxyInput(config.proxy_mode.url);
      setProxyError('');
    } else {
      setProxyInput('');
      setProxyError('');
    }
  }, [config?.proxy_mode]);

  const handleProxyInputChange = (val: string) => {
    setProxyInput(val);
    if (!val.trim()) {
      setProxyError('');
      return;
    }
    const result = validateProxyUrl(val, t);
    setProxyError(result.valid ? '' : result.error || t('settings.items.proxy.formatError'));
  };

  const handleProxyInputBlur = () => {
    if (!proxyInput.trim()) {
      setProxyError(t('settings.items.proxy.emptyError'));
      return;
    }
    const result = validateProxyUrl(proxyInput, t);
    if (result.valid && result.normalized) {
      setProxyError('');
      updateConfig({
        proxy_mode: {
          type: 'Custom',
          url: result.normalized,
        },
      });
    } else {
      setProxyError(result.error || t('settings.items.proxy.formatError'));
    }
  };

  return (
    <Drawer
      side="right"
      title={
        <div className="flex h-5 items-center gap-1.5">
          <span className="text-base font-semibold tracking-wider text-primary leading-none">
            {t('settings.title')}
          </span>
          <GameAutoSaveBadge triggerId={saveTriggerId} />
        </div>
      }
      subtitle={t('settings.subtitle')}
      width="360px"
      onClose={onClose}
      bodyClassName="px-2.5 pb-5 pt-1 space-y-3 select-none"
      headerExtra={
        <Button
          variant="ghost"
          size="xs"
          onClick={resetToDefaults}
          className="text-xs text-white/45 hover:text-white"
        >
          <RotateCcw size={11} className="shrink-0" />
          <span>{t('settings.resetBtn')}</span>
        </Button>
      }
    >
      <SettingSection title={t('settings.sections.appearance')}>
        <SettingRow
          label={t('settings.items.language.label')}
          tip={t('settings.items.language.tip')}
        >
          <Select
            value={config?.language ?? 'en'}
            options={[
              ['en', 'English'],
              ['zh-TW', '繁體中文'],
              ['zh-CN', '简体中文'],
              ['ja', '日本語'],
            ]}
            onChange={(language) => updateConfig({ language: language as Language })}
          />
        </SettingRow>
      </SettingSection>

      <SettingSection title={t('settings.sections.download')}>
        <SettingRow
          label={t('settings.items.maxConcurrent.label')}
          tip={t('settings.items.maxConcurrent.tip')}
        >
          <Select
            value={String(config?.max_concurrent_tasks ?? 3)}
            options={[
              ['1', `${t('settings.items.maxConcurrent.opt1')}`],
              ['2', `${t('settings.items.maxConcurrent.opt2')}`],
              ['3', `${t('settings.items.maxConcurrent.opt3')}`],
              ['5', `${t('settings.items.maxConcurrent.opt5')}`],
              ['8', `${t('settings.items.maxConcurrent.opt8')}`],
            ]}
            onChange={(value) => updateConfig({ max_concurrent_tasks: Number(value) })}
          />
        </SettingRow>

        <SettingRow
          label={t('settings.items.bandwidthLimit.label')}
          tip={t('settings.items.bandwidthLimit.tip')}
        >
          <SpeedControl
            speedBytes={config?.max_download_speed ?? 0}
            onChange={(max_download_speed) => updateConfig({ max_download_speed })}
          />
        </SettingRow>

        <SettingRow
          label={t('settings.items.preferredQuality.label')}
          tip={t('settings.items.preferredQuality.tip')}
        >
          <Select<PreferredQuality>
            value={config?.preferred_quality ?? 'highest'}
            options={[
              ['highest', t('settings.items.preferredQuality.highest')],
              ['1080p', t('settings.items.preferredQuality.p1080')],
              ['720p', t('settings.items.preferredQuality.p720')],
              ['480p', t('settings.items.preferredQuality.p480')],
              ['lowest', t('settings.items.preferredQuality.lowest')],
            ]}
            onChange={(preferred_quality) => updateConfig({ preferred_quality })}
          />
        </SettingRow>

        <SettingRow
          leading={<DiskSpaceRing path={config?.download_dir} />}
          label={t('settings.items.downloadDir.label')}
          subtitle={config?.download_dir}
          tip={t('settings.items.downloadDir.tip')}
        >
          <div className="flex items-center gap-1.5">
            <Button
              variant="secondary"
              size="xs"
              onClick={openDownloadDir}
              title={t('settings.items.downloadDir.openTitle')}
              className="text-xs text-white/80 hover:text-white"
            >
              <FolderOpen size={12} className="text-white/60 shrink-0" />
              <span>{t('settings.items.downloadDir.open')}</span>
            </Button>
            <Button
              variant="secondary"
              size="xs"
              onClick={selectDir}
              title={t('settings.items.downloadDir.changeTitle')}
              className="text-xs text-white/80 hover:text-white"
            >
              <FolderSync size={12} className="text-white/60 shrink-0" />
              <span>{t('settings.items.downloadDir.change')}</span>
            </Button>
          </div>
        </SettingRow>

        <SettingRow
          label={t('settings.items.deleteMode.label')}
          tip={t('settings.items.deleteMode.tip')}
        >
          <Select
            value={deleteFileMode}
            options={[
              ['Ask', t('settings.items.deleteMode.ask')],
              ['Always', t('settings.items.deleteMode.always')],
              ['Never', t('settings.items.deleteMode.never')],
            ]}
            onChange={(mode) => handleSetDeleteFileMode(mode as DeleteFileMode)}
          />
        </SettingRow>
      </SettingSection>

      <SettingSection title={t('settings.sections.network')}>
        <SettingRow label={t('settings.items.proxy.label')} tip={t('settings.items.proxy.tip')}>
          <Select
            value={config?.proxy_mode ? config.proxy_mode.type : 'Direct'}
            options={[
              ['Direct', t('settings.items.proxy.direct')],
              ['System', t('settings.items.proxy.system')],
              ['Custom', t('settings.items.proxy.custom')],
            ]}
            onChange={(value) => {
              if (value === 'Custom') {
                const currentUrl =
                  proxyInput ||
                  (config?.proxy_mode?.type === 'Custom'
                    ? config.proxy_mode.url
                    : 'http://127.0.0.1:7890');
                const result = validateProxyUrl(currentUrl, t);
                if (result.valid && result.normalized) {
                  setProxyError('');
                  updateConfig({
                    proxy_mode: {
                      type: 'Custom',
                      url: result.normalized,
                    },
                  });
                } else {
                  setProxyInput(currentUrl);
                  setProxyError(result.error || t('settings.items.proxy.formatError'));
                }
              } else {
                setProxyError('');
                updateConfig({
                  proxy_mode: { type: value as 'System' | 'Direct' },
                });
              }
            }}
          />
        </SettingRow>
        {config?.proxy_mode?.type === 'Custom' && (
          <SettingRow
            label={t('settings.items.proxy.endpointLabel')}
            tip={t('settings.items.proxy.endpointTip')}
          >
            <div className="flex flex-col items-end gap-1">
              <input
                className={`input-field h-6 w-44 rounded px-2 text-xs ${
                  proxyError ? 'border-rose-500/50 bg-rose-500/10 text-rose-200' : ''
                }`}
                placeholder="http://127.0.0.1:7890"
                spellCheck={false}
                value={proxyInput}
                onChange={(e) => handleProxyInputChange(e.target.value)}
                onBlur={handleProxyInputBlur}
              />
              {proxyError && <span className="text-meta-sub text-rose-300">{proxyError}</span>}
            </div>
          </SettingRow>
        )}
      </SettingSection>

      <SettingSection title={t('settings.sections.system')}>
        <SettingRow
          label={t('settings.items.autoUpdate.label')}
          tip={t('settings.items.autoUpdate.tip')}
        >
          <Toggle
            checked={config?.auto_check_update ?? true}
            onChange={(auto_check_update) => updateConfig({ auto_check_update })}
          />
        </SettingRow>
        <SettingRow
          label={t('settings.items.checkUpdate.label')}
          tip={t('settings.items.checkUpdate.tip')}
        >
          <Button
            variant="secondary"
            size="xs"
            disabled={isCheckingUpdate}
            onClick={handleCheckUpdate}
          >
            <RefreshCw size={12} className={`shrink-0 ${isCheckingUpdate ? 'animate-spin' : ''}`} />
            <span>{isCheckingUpdate ? t('settings.items.checkUpdate.checking') : t('settings.items.checkUpdate.button')}</span>
          </Button>
        </SettingRow>
        <SettingRow label={t('settings.items.logging.label')} tip={t('settings.items.logging.tip')}>
          <Toggle
            checked={config?.enable_logging ?? true}
            onChange={(enable_logging) => updateConfig({ enable_logging })}
          />
        </SettingRow>
      </SettingSection>
    </Drawer>
  );
}

export default SettingsDrawer;
