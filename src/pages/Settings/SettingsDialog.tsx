import React, { useState, useEffect } from 'react';
import {
  Dialog,
  DialogSurface,
  DialogTitle,
  DialogBody,
  DialogContent,
  DialogActions,
  Button,
  Switch,
  Input,
  Text,
  Label,
  Dropdown,
  Option,
  Slider,
  MessageBar,
  MessageBarBody,
  TabList,
  Tab,
  Textarea,
  InfoLabel,
} from '@fluentui/react-components';
import { useAppStore } from '../../stores/appStore';
import {
  Shield24Regular,
  Person24Regular,
  PuzzlePiece24Regular,
  ArrowSync24Regular,
  Database24Regular,
  PaintBrush24Regular,
  Home24Regular,
  Tag24Regular,
  ArrowDownload24Regular,
  Bug24Regular,
} from '@fluentui/react-icons';
import type { AppSettingsTab, CaptchaMode, AppTheme } from '../../types';
import * as tauri from '../../services/tauri';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import {
  PageEnterMotion,
  SectionEnterMotion,
} from '../../components/Common/motion';

type SettingsTab = AppSettingsTab;
type IdentityStartupMode = 'last_used' | 'configured_default';

function normalizeSyncMaxPages(value?: number): number {
  if (!value || value < 10) {
    return 100;
  }
  return value;
}

export const SettingsDialog: React.FC = () => {
  const showSettingsDialog = useAppStore((s) => s.showSettingsDialog);
  const setShowSettingsDialog = useAppStore((s) => s.setShowSettingsDialog);
  const settingsDialogTab = useAppStore((s) => s.settingsDialogTab);
  const config = useAppStore((s) => s.config);
  const theme = useAppStore((s) => s.theme);
  const setTheme = useAppStore((s) => s.setTheme);
  const loadConfig = useAppStore((s) => s.loadConfig);
  const identities = useAppStore((s) => s.identities);
  const currentIdentity = useAppStore((s) => s.currentIdentity);
  const accounts = useAppStore((s) => s.accounts);
  const loadBills = useAppStore((s) => s.loadBills);
  const loadAccounts = useAppStore((s) => s.loadAccounts);

  const [selectedTab, setSelectedTab] = useState<SettingsTab>('ui');
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState('');

  // Security settings
  const [startupProtection, setStartupProtection] = useState(
    config?.security.enable_startup_protection ?? false
  );
  const [protectionPassword, setProtectionPassword] = useState('');
  const [passwordChanged, setPasswordChanged] = useState(false);

  // Identity settings
  const [identityStartupMode, setIdentityStartupMode] = useState<IdentityStartupMode>(
    config?.identity.remember_default ? 'configured_default' : 'last_used'
  );

  // Captcha settings
  const [captchaMode, setCaptchaMode] = useState<CaptchaMode>(
    config?.captcha.mode ?? 'manual'
  );
  const [ocrHost, setOcrHost] = useState(config?.captcha.remote_ocr_host ?? '');
  const [ocrPort, setOcrPort] = useState(
    config?.captcha.remote_ocr_port ? String(config.captcha.remote_ocr_port) : ''
  );
  const [ocrHttpUrl, setOcrHttpUrl] = useState(config?.captcha.remote_ocr_http_url || 'http://127.0.0.1:5000');
  const [ocrRetry, setOcrRetry] = useState(config?.captcha.ocr_retry_count ?? 5);

  // Sync settings
  const [maxPages, setMaxPages] = useState(normalizeSyncMaxPages(config?.sync.max_pages));
  const [earlyStop, setEarlyStop] = useState(config?.sync.early_stop_threshold ?? 5);
  const [skipGraduatedAccounts, setSkipGraduatedAccounts] = useState(config?.sync.skip_graduated_accounts ?? true);
  const [autoMerge, setAutoMerge] = useState(config?.sync.auto_merge_after_sync ?? true);
  const [autoSyncEnabled, setAutoSyncEnabled] = useState(config?.sync.auto_sync_enabled ?? false);
  const [autoSyncIntervalMinutes, setAutoSyncIntervalMinutes] = useState(config?.sync.auto_sync_interval_minutes ?? 60);
  const [autoSyncRange, setAutoSyncRange] = useState<tauri.SyncRangePreset>(config?.sync.auto_sync_range ?? 'month');

  // Data settings
  const [dataDir, setDataDir] = useState(config?.data.data_directory || 'Data');
  const [snapshotKeep, setSnapshotKeep] = useState(config?.data.snapshot_keep_count ?? 10);
  const [rulesUpdateUrl, setRulesUpdateUrl] = useState(config?.classification.rules_update_url ?? '');
  const [rulesPath, setRulesPath] = useState(config?.classification.rules_path ?? '');

  // Debug settings
  const [debugMessage, setDebugMessage] = useState('');
  const [debugResponse, setDebugResponse] = useState('');
  const [debugTesting, setDebugTesting] = useState(false);
  const [selectedAccountId, setSelectedAccountId] = useState('');
  const [repairingIdentity, setRepairingIdentity] = useState(false);
  const [repairingAccount, setRepairingAccount] = useState(false);
  const showError = useAppStore((s) => s.showError);

  // UI settings
  const [decimalPlaces, setDecimalPlaces] = useState(config?.ui.decimal_places ?? 2);
  const [homeTrendRange, setHomeTrendRange] = useState(config?.ui.home_trend_range ?? 'week');
  const [homeCategoryRange, setHomeCategoryRange] = useState(config?.ui.home_category_range ?? 'month');
  const [autoCheckUpdate, setAutoCheckUpdate] = useState(config?.update.auto_check ?? true);
  const [checkIntervalHours, setCheckIntervalHours] = useState(config?.update.check_interval_hours ?? 24);

  useEffect(() => {
    if (!config) return;
    setStartupProtection(config.security.enable_startup_protection);
    setIdentityStartupMode(config.identity.remember_default ? 'configured_default' : 'last_used');
    setCaptchaMode(config.captcha.mode ?? 'manual');
    setOcrHost(config.captcha.remote_ocr_host ?? '');
    setOcrPort(config.captcha.remote_ocr_port ? String(config.captcha.remote_ocr_port) : '');
    setOcrHttpUrl(config.captcha.remote_ocr_http_url || 'http://127.0.0.1:5000');
    setOcrRetry(config.captcha.ocr_retry_count || (config.captcha.mode !== 'manual' ? 5 : 0));
    setMaxPages(normalizeSyncMaxPages(config.sync.max_pages));
    setEarlyStop(config.sync.early_stop_threshold ?? 5);
    setSkipGraduatedAccounts(config.sync.skip_graduated_accounts ?? true);
    setAutoMerge(config.sync.auto_merge_after_sync ?? true);
    setAutoSyncEnabled(config.sync.auto_sync_enabled ?? false);
    setAutoSyncIntervalMinutes(config.sync.auto_sync_interval_minutes ?? 60);
    setAutoSyncRange(config.sync.auto_sync_range ?? 'month');
    setDataDir(config.data.data_directory || 'Data');
    setSnapshotKeep(config.data.snapshot_keep_count ?? 10);
    setRulesUpdateUrl(config.classification.rules_update_url ?? '');
    setRulesPath(config.classification.rules_path ?? '');
    setDecimalPlaces(config.ui.decimal_places ?? 2);
    setHomeTrendRange(config.ui.home_trend_range ?? 'week');
    setHomeCategoryRange(config.ui.home_category_range ?? 'month');
    setAutoCheckUpdate(config.update.auto_check ?? true);
    setCheckIntervalHours(config.update.check_interval_hours ?? 24);
  }, [config]);

  useEffect(() => {
    if (!showSettingsDialog) {
      return;
    }
    setSelectedTab(settingsDialogTab);
  }, [settingsDialogTab, showSettingsDialog]);

  const currentDefaultIdentity =
    identities.find((identity) => identity.id === config?.identity.default_identity_id) ?? null;

  const persistSettings = async (closeAfterSave: boolean) => {
    if (!config) return;

    setSaving(true);
    setMessage('');
    try {
      const nextConfig: tauri.AppConfig = {
        ...config,
        security: {
          enable_startup_protection: startupProtection,
          password_hash: config.security.password_hash,
        },
        identity: {
          ...config.identity,
          remember_default: identityStartupMode === 'configured_default',
        },
        captcha: {
          mode: captchaMode,
          remote_ocr_host: ocrHost,
          remote_ocr_port: parseInt(ocrPort) || 0,
          remote_ocr_http_url: ocrHttpUrl,
          onnx_model_path: config.captcha.onnx_model_path,
          ocr_retry_count: ocrRetry,
        },
        sync: {
          max_pages: normalizeSyncMaxPages(maxPages),
          early_stop_threshold: earlyStop,
          skip_graduated_accounts: skipGraduatedAccounts,
          auto_merge_after_sync: autoMerge,
          auto_sync_enabled: autoSyncEnabled,
          auto_sync_interval_minutes: autoSyncIntervalMinutes,
          auto_sync_range: autoSyncRange,
        },
        data: {
          data_directory: dataDir,
          snapshot_keep_count: snapshotKeep,
        },
        classification: {
          rules_path: rulesPath,
          rules_update_url: rulesUpdateUrl,
        },
        update: {
          ...config.update,
          auto_check: autoCheckUpdate,
          check_interval_hours: checkIntervalHours,
        },
        ui: {
          theme,
          language: config.ui.language,
          decimal_places: decimalPlaces,
          home_trend_range: homeTrendRange,
          home_category_range: homeCategoryRange,
        },
      };

      await tauri.save_config(nextConfig);

      if (startupProtection && passwordChanged && protectionPassword.trim()) {
        await tauri.set_startup_password(protectionPassword.trim());
      }

      await loadConfig();
      setMessage('设置已保存');
      if (closeAfterSave) {
        setShowSettingsDialog(false);
      }
    } catch (e) {
      setMessage('保存失败');
      console.error('Failed to save config:', e);
    } finally {
      setSaving(false);
    }
  };

  const handleApply = async () => {
    await persistSettings(false);
  };

  const handleSaveAndClose = async () => {
    await persistSettings(true);
  };

  const renderContent = () => {
    switch (selectedTab) {
      case 'security':
        return (
          <div style={{ display: 'grid', gap: 16 }}>
            <Text weight="semibold" size={400}>安全设置</Text>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <div>
                <InfoLabel info="启用后，每次启动应用前都必须输入启动密码。适合共享电脑或希望保护本地账单数据的场景。">
                  启动保护
                </InfoLabel>
                <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
                  开启后每次启动需要输入密码
                </Text>
              </div>
              <Switch
                checked={startupProtection}
                onChange={(_, data) => setStartupProtection(data.checked)}
              />
            </div>
            {startupProtection && (
              <div>
                <InfoLabel info="仅在你修改密码时生效。留空不会覆盖当前已保存的启动密码。">
                  保护密码
                </InfoLabel>
                <Input
                  type="password"
                  value={protectionPassword}
                  placeholder="设置保护密码"
                  onChange={(e) => { setProtectionPassword(e.currentTarget.value); setPasswordChanged(true); }}
                  style={{ width: '100%' }}
                />
              </div>
            )}
          </div>
        );

      case 'identity':
        return (
          <div style={{ display: 'grid', gap: 16 }}>
            <Text weight="semibold" size={400}>身份设置</Text>
            <div>
              <InfoLabel info="决定应用启动后优先尝试进入哪个身份。若只有一个启用身份，会直接进入该身份。">
                启动时优先加载
              </InfoLabel>
              <Dropdown
                value={identityStartupMode === 'last_used' ? '上一次使用的身份' : '设置的默认身份'}
                selectedOptions={[identityStartupMode]}
                onOptionSelect={(_, data) => setIdentityStartupMode(data.optionValue as IdentityStartupMode)}
                style={{ width: '100%' }}
              >
                <Option value="last_used">上一次使用的身份</Option>
                <Option value="configured_default">设置的默认身份</Option>
              </Dropdown>
            </div>
            <div
              style={{
                padding: 14,
                borderRadius: 10,
                border: '1px solid var(--colorNeutralStroke2)',
                background: 'var(--colorNeutralBackground2)',
              }}
            >
              <Text weight="semibold" block style={{ marginBottom: 4 }}>
                当前默认身份
              </Text>
              <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
                {currentDefaultIdentity
                  ? `${currentDefaultIdentity.name}（ID #${currentDefaultIdentity.id}）`
                  : '尚未设置。可在“切换身份”对话框中直接设为默认身份。'}
              </Text>
            </div>
            <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
              如果当前只有一个启用身份，应用会直接进入该身份，不受上述策略影响。
            </Text>
          </div>
        );

      case 'captcha':
        return (
          <div style={{ display: 'grid', gap: 16 }}>
            <Text weight="semibold" size={400}>验证码设置</Text>
            <div>
              <InfoLabel info="手动输入最稳定；远程 OCR 适合已有识别服务；本地 ONNX 适合离线识别。不同模式对速度和成功率影响较大。">
                识别模式
              </InfoLabel>
              <Dropdown
                value={
                  captchaMode === 'manual' ? '手动输入'
                    : captchaMode === 'remote_ocr' ? '远程OCR(旧)'
                    : captchaMode === 'remote_ocr_http' ? '远程OCR(RESTful)'
                    : '本地ONNX'
                }
                selectedOptions={[captchaMode]}
                onOptionSelect={(_, data) => {
                    const mode = data.optionValue as CaptchaMode;
                    setCaptchaMode(mode);
                    if (mode !== 'manual' && ocrRetry === 0) setOcrRetry(5);
                  }}
                style={{ width: '100%' }}
              >
                <Option value="manual">手动输入</Option>
                <Option value="remote_ocr">远程OCR(旧)</Option>
                <Option value="remote_ocr_http">远程OCR(RESTful)</Option>
                <Option value="local_onnx">本地ONNX</Option>
              </Dropdown>
            </div>
            {captchaMode === 'remote_ocr' && (
              <>
                <div>
                  <InfoLabel info="旧版 OCR 服务的主机地址，不包含协议和端口，例如 192.168.1.100。">
                    OCR服务器地址
                  </InfoLabel>
                  <Input
                    value={ocrHost}
                    onChange={(e) => setOcrHost(e.currentTarget.value)}
                    placeholder="如: 192.168.1.100"
                    style={{ width: '100%' }}
                  />
                </div>
                <div>
                  <InfoLabel info="旧版 OCR 服务监听的端口，例如 8888。">
                    OCR服务器端口
                  </InfoLabel>
                  <Input
                    value={ocrPort}
                    onChange={(e) => setOcrPort(e.currentTarget.value)}
                    placeholder="如: 8888"
                    style={{ width: '100%' }}
                  />
                </div>
              </>
            )}
            {captchaMode === 'remote_ocr_http' && (
              <div>
                <InfoLabel info="RESTful OCR 接口完整地址，通常形如 http://127.0.0.1:5000。">
                  RESTful OCR 服务地址
                </InfoLabel>
                <Input
                  value={ocrHttpUrl}
                  onChange={(e) => setOcrHttpUrl(e.currentTarget.value)}
                  placeholder="如: http://127.0.0.1:5000"
                  style={{ width: '100%' }}
                />
              </div>
            )}
            {captchaMode !== 'manual' && (
              <div>
                <InfoLabel info="验证码识别失败后自动重新尝试的次数。次数越高越稳，但登录耗时也会更长。">
                  验证码错误重试次数: {ocrRetry}
                </InfoLabel>
                <Slider
                  min={1}
                  max={20}
                  value={ocrRetry}
                  onChange={(_, data) => setOcrRetry(data.value)}
                />
                <Text block size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
                  登录时验证码识别错误后
                </Text>
                <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
                  自动重试的最大次数
                </Text>
              </div>
            )}
          </div>
        );

      case 'sync':
        return (
          <div style={{ display: 'grid', gap: 16 }}>
            <Text weight="semibold" size={400}>同步设置</Text>
            <div>
              <InfoLabel info="单次同步最多抓取的账单页数。数值越大，补历史账单越完整，但同步时间也越长。旧配置中的 0 会自动按默认 100 处理。">
                默认同步页数上限: {maxPages}
              </InfoLabel>
              <Slider
                min={10}
                max={500}
                step={10}
                value={maxPages}
                onChange={(_, data) => setMaxPages(data.value)}
              />
            </div>
            <div>
              <InfoLabel info="连续遇到旧数据时，提前结束同步的阈值。阈值越小，同步越快；越大，越适合补录边界日期附近的数据。">
                提前停止阈值: {earlyStop}
              </InfoLabel>
              <Slider
                min={1}
                max={20}
                value={earlyStop}
                onChange={(_, data) => setEarlyStop(data.value)}
              />
            </div>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <div>
                <InfoLabel info="启用后，毕业日期早于今天的账号会在同步阶段被自动跳过。毕业日期为空（至今）或设置为未来时间的账号仍会正常同步。">
                  跳过已毕业账号同步
                </InfoLabel>
                <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
                  适合不再使用校园卡的历史账号
                </Text>
              </div>
              <Switch checked={skipGraduatedAccounts} onChange={(_, data) => setSkipGraduatedAccounts(data.checked)} />
            </div>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <div>
                <InfoLabel info="同步成功后，自动把新抓取的原始账单并入合并账单表。通常建议开启。">
                  同步后自动合并
                </InfoLabel>
                <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
                  自动将新条目追加到合并数据表
                </Text>
              </div>
              <Switch checked={autoMerge} onChange={(_, data) => setAutoMerge(data.checked)} />
            </div>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <div>
                <InfoLabel info="启用后，应用会在后台按设定周期对默认身份执行账单增量同步。手动验证码模式下会自动跳过。">
                  启用定时账单同步
                </InfoLabel>
                <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
                  仅对默认身份生效，建议配合自动识别验证码模式使用
                </Text>
              </div>
              <Switch checked={autoSyncEnabled} onChange={(_, data) => setAutoSyncEnabled(data.checked)} />
            </div>
            <div>
              <InfoLabel info="后台自动同步的时间间隔，单位分钟。">
                定时同步间隔: {autoSyncIntervalMinutes} 分钟
              </InfoLabel>
              <Slider
                min={5}
                max={720}
                step={5}
                value={autoSyncIntervalMinutes}
                onChange={(_, data) => setAutoSyncIntervalMinutes(data.value)}
                disabled={!autoSyncEnabled}
              />
            </div>
            <div>
              <InfoLabel info="每次后台自动同步时使用的账单时间范围。">
                定时同步范围
              </InfoLabel>
              <Dropdown
                value={
                  autoSyncRange === 'week' ? '最近一周'
                    : autoSyncRange === 'half_month' ? '最近半个月'
                    : autoSyncRange === 'month' ? '最近一个月'
                    : autoSyncRange === 'half_year' ? '最近半年'
                    : autoSyncRange === 'year' ? '最近一年'
                    : '全部'
                }
                selectedOptions={[autoSyncRange]}
                onOptionSelect={(_, data) => setAutoSyncRange(data.optionValue as tauri.SyncRangePreset)}
                disabled={!autoSyncEnabled}
                style={{ width: '100%' }}
              >
                <Option value="week">最近一周</Option>
                <Option value="half_month">最近半个月</Option>
                <Option value="month">最近一个月</Option>
                <Option value="half_year">最近半年</Option>
                <Option value="year">最近一年</Option>
                <Option value="all">全部</Option>
              </Dropdown>
            </div>
          </div>
        );

      case 'data':
        return (
          <div style={{ display: 'grid', gap: 16 }}>
            <Text weight="semibold" size={400}>数据设置</Text>
            <div>
              <InfoLabel info="应用存放数据库、配置和快照的目录。修改后应确保新目录可读写，并注意手动迁移旧数据。">
                数据目录
              </InfoLabel>
              <div style={{ display: 'flex', gap: 8 }}>
                <Input
                  value={dataDir}
                  onChange={(e) => setDataDir(e.currentTarget.value)}
                  style={{ flex: 1 }}
                />
                <Button
                  appearance="subtle"
                  onClick={async () => {
                    const selected = await openDialog({ directory: true, multiple: false });
                    if (selected) setDataDir(typeof selected === 'string' ? selected : selected);
                  }}
                >
                  浏览
                </Button>
              </div>
            </div>
            <div>
              <InfoLabel info="创建新快照后，系统自动保留的最近快照数量。超过数量的旧快照会被自动清理。">
                快照自动保留数: {snapshotKeep}
              </InfoLabel>
              <Slider
                min={1}
                max={50}
                value={snapshotKeep}
                onChange={(_, data) => setSnapshotKeep(data.value)}
              />
            </div>
            <div
              style={{
                padding: 14,
                borderRadius: 10,
                border: '1px solid var(--colorNeutralStroke2)',
                background: 'var(--colorNeutralBackground2)',
                display: 'grid',
                gap: 12,
              }}
            >
              <Text weight="semibold">数据修复</Text>
              <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
                使用交易号去重，修复因历史同步 Bug 导致的重复账单。身份级去重针对合并账单，账号级去重针对原始账单。
              </Text>
              <div style={{ display: 'grid', gap: 8 }}>
                <Button
                  appearance="primary"
                  disabled={!currentIdentity || repairingIdentity}
                  onClick={async () => {
                    if (!currentIdentity) return;
                    setRepairingIdentity(true);
                    setMessage('');
                    try {
                      const result = await tauri.dedupe_identity_bills(currentIdentity.id);
                      await loadBills();
                      setMessage(`身份级去重完成：补正 ${result.backfilled_count} 条，删除 ${result.removed_count} 条重复记录`);
                    } catch (e) {
                      setMessage(`身份级去重失败: ${e}`);
                    } finally {
                      setRepairingIdentity(false);
                    }
                  }}
                >
                  {repairingIdentity ? '身份级去重中...' : '身份级别去重'}
                </Button>
                <Dropdown
                  value={selectedAccountId || '选择账号后执行账号级去重'}
                  selectedOptions={selectedAccountId ? [selectedAccountId] : []}
                  onOptionSelect={(_, data) => setSelectedAccountId(data.optionValue ?? '')}
                  disabled={!currentIdentity || accounts.length === 0 || repairingAccount}
                  style={{ width: '100%' }}
                >
                  {accounts.map((account) => {
                    const label = `${account.account_name}（${account.account_id}）`;
                    return (
                      <Option key={account.account_id} value={account.account_id} text={label}>
                        {label}
                      </Option>
                    );
                  })}
                </Dropdown>
                <Button
                  appearance="secondary"
                  disabled={!currentIdentity || !selectedAccountId || repairingAccount}
                  onClick={async () => {
                    if (!currentIdentity || !selectedAccountId) return;
                    setRepairingAccount(true);
                    setMessage('');
                    try {
                      const result = await tauri.dedupe_account_bills(currentIdentity.id, selectedAccountId);
                      await loadAccounts(currentIdentity.id);
                      await loadBills();
                      setMessage(`账号级去重完成：补正 ${result.backfilled_count} 条，删除 ${result.removed_count} 条重复记录`);
                    } catch (e) {
                      setMessage(`账号级去重失败: ${e}`);
                    } finally {
                      setRepairingAccount(false);
                    }
                  }}
                >
                  {repairingAccount ? '账号级去重中...' : '账号级别去重'}
                </Button>
              </div>
            </div>
          </div>
        );

      case 'ui':
        return (
          <div style={{ display: 'grid', gap: 16 }}>
            <Text weight="semibold" size={400}>界面设置</Text>
            <div>
              <InfoLabel info="控制应用使用亮色、暗色，或跟随系统主题。">
                主题
              </InfoLabel>
              <Dropdown
                value={theme === 'light' ? '亮色' : theme === 'dark' ? '暗色' : '跟随系统'}
                selectedOptions={[theme]}
                onOptionSelect={(_, data) => setTheme(data.optionValue as AppTheme)}
                style={{ width: '100%' }}
              >
                <Option value="light">亮色</Option>
                <Option value="dark">暗色</Option>
                <Option value="system">跟随系统</Option>
              </Dropdown>
            </div>
            <div>
              <InfoLabel info="控制统计页面和图表里金额数据保留的小数位数。">
                统计小数位数: {decimalPlaces}
              </InfoLabel>
              <Slider
                min={0}
                max={6}
                step={1}
                value={decimalPlaces}
                onChange={(_, data) => setDecimalPlaces(data.value)}
              />
              <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
                控制统计页面中金额数值的保留小数位数
              </Text>
            </div>
          </div>
        );

      case 'home':
        return (
          <div style={{ display: 'grid', gap: 16 }}>
            <Text weight="semibold" size={400}>首页图表设置</Text>
            <div>
              <InfoLabel info="控制首页消费趋势图默认展示的时间范围。">
                趋势图表范围
              </InfoLabel>
              <Dropdown
                value={
                  homeTrendRange === 'today' ? '今天' :
                  homeTrendRange === 'week' ? '本周' :
                  homeTrendRange === 'recent7days' ? '最近7天' :
                  homeTrendRange === 'month' ? '本月' : '本周'
                }
                selectedOptions={[homeTrendRange]}
                onOptionSelect={(_, data) => setHomeTrendRange(data.optionValue ?? 'week')}
                style={{ width: '100%' }}
              >
                <Option value="today">今天</Option>
                <Option value="week">本周</Option>
                <Option value="recent7days">最近7天</Option>
                <Option value="month">本月</Option>
              </Dropdown>
              <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
                首页消费趋势图的时间范围
              </Text>
            </div>
            <div>
              <InfoLabel info="控制首页分类占比图默认统计的时间范围。">
                分类图表范围
              </InfoLabel>
              <Dropdown
                value={
                  homeCategoryRange === 'today' ? '今天' :
                  homeCategoryRange === 'week' ? '本周' :
                  homeCategoryRange === 'recent7days' ? '最近7天' :
                  homeCategoryRange === 'month' ? '本月' : '本月'
                }
                selectedOptions={[homeCategoryRange]}
                onOptionSelect={(_, data) => setHomeCategoryRange(data.optionValue ?? 'month')}
                style={{ width: '100%' }}
              >
                <Option value="today">今天</Option>
                <Option value="week">本周</Option>
                <Option value="recent7days">最近7天</Option>
                <Option value="month">本月</Option>
              </Dropdown>
              <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
                首页消费分类占比图的时间范围
              </Text>
            </div>
          </div>
        );

      case 'classification':
        return (
          <div style={{ display: 'grid', gap: 16 }}>
            <Text weight="semibold" size={400}>分类规则设置</Text>
            <div>
              <InfoLabel info="本地分类规则文件路径。应用会优先使用这里的 TOML 规则进行分类、位置映射和时段判断。">
                分类规则文件路径
              </InfoLabel>
              <div style={{ display: 'flex', gap: 8 }}>
                <Input
                  value={rulesPath}
                  onChange={(e) => setRulesPath(e.currentTarget.value)}
                  placeholder="Data/classification_rules.toml"
                  style={{ flex: 1 }}
                />
                <Button
                  appearance="subtle"
                  onClick={async () => {
                    const selected = await openDialog({
                      filters: [{ name: 'TOML 文件', extensions: ['toml'] }],
                      multiple: false,
                    });
                    if (selected) setRulesPath(typeof selected === 'string' ? selected : selected);
                  }}
                >
                  浏览
                </Button>
              </div>
            </div>
            <div>
              <InfoLabel info="远程规则更新地址。用于从 GitHub 或其他 HTTP 源拉取最新版分类规则。">
                规则更新源(GitHub)
              </InfoLabel>
              <Input
                value={rulesUpdateUrl}
                onChange={(e) => setRulesUpdateUrl(e.currentTarget.value)}
                placeholder="https://github.com/..."
                style={{ width: '100%' }}
              />
            </div>
          </div>
        );

      case 'update':
        return (
          <div style={{ display: 'grid', gap: 16 }}>
            <Text weight="semibold" size={400}>更新设置</Text>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <InfoLabel info="启用后，应用会按设定时间间隔自动检查新版本。">
                自动检查更新
              </InfoLabel>
              <Switch checked={autoCheckUpdate} onChange={(_, data) => setAutoCheckUpdate(data.checked)} />
            </div>
            <div>
              <InfoLabel info="自动检查更新的时间间隔，单位为小时。1 表示每小时检查一次，168 表示每周一次。">
                检查间隔(小时): {checkIntervalHours}
              </InfoLabel>
              <Slider min={1} max={168} value={checkIntervalHours} onChange={(_, data) => setCheckIntervalHours(data.value)} />
            </div>
          </div>
        );

      case 'debug':
        return (
          <div style={{ display: 'grid', gap: 16 }}>
            <Text weight="semibold" size={400}>调试面板</Text>
            <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
              测试错误上报功能。输入错误信息后点击"触发错误"，错误会被发送到后端并显示错误对话框。
            </Text>
            <div>
              <InfoLabel info="输入一段测试用错误文本，用于验证前端错误记录和错误弹窗是否工作正常。">
                错误信息
              </InfoLabel>
              <Textarea
                value={debugMessage}
                onChange={(_, data) => setDebugMessage(data.value)}
                placeholder="输入要测试的错误信息..."
                style={{ width: '100%', minHeight: 80 }}
              />
            </div>
            <Button
              appearance="primary"
              onClick={async () => {
                if (!debugMessage.trim()) {
                  setDebugResponse('请输入错误信息');
                  return;
                }
                setDebugTesting(true);
                setDebugResponse('');
                try {
                  await tauri.log_error(debugMessage);
                  setDebugResponse('✓ 错误已发送到后端');
                  showError(debugMessage);
                } catch (e) {
                  setDebugResponse(`✗ 发送失败: ${e}`);
                } finally {
                  setDebugTesting(false);
                }
              }}
              disabled={debugTesting}
            >
              {debugTesting ? '发送中...' : '触发错误'}
            </Button>
            {debugResponse && (
              <MessageBar intent={debugResponse.startsWith('✓') ? 'success' : 'error'}>
                <MessageBarBody>{debugResponse}</MessageBarBody>
              </MessageBar>
            )}
            <div
              style={{
                marginTop: 16,
                padding: 12,
                borderRadius: 8,
                background: 'var(--colorNeutralBackground4)',
              }}
            >
              <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
                提示：错误日志保存在 ~/.local/share/cn.edu.shmtu.terminal.tauri/frontend_errors.log
              </Text>
            </div>
          </div>
        );
    }
  };

  return (
    <Dialog open={showSettingsDialog} onOpenChange={(_, data) => !data.open && setShowSettingsDialog(false)}>
      <DialogSurface style={{ maxWidth: 700 }}>
        <DialogBody>
          <DialogTitle>设置</DialogTitle>
          <DialogContent>
            <div style={{ display: 'grid', gridTemplateColumns: '160px 1fr', gap: 16, minHeight: 350 }}>
              {/* Left Nav */}
              <SectionEnterMotion>
                <div>
                  <TabList
                    vertical
                    selectedValue={selectedTab}
                    onTabSelect={(_, data) => setSelectedTab(data.value as SettingsTab)}
                  >
                    <Tab icon={<PaintBrush24Regular />} value="ui">界面</Tab>
                    <Tab icon={<Home24Regular />} value="home">首页图表</Tab>
                    <Tab icon={<Person24Regular />} value="identity">身份</Tab>
                    <Tab icon={<Shield24Regular />} value="security">安全</Tab>
                    <Tab icon={<ArrowSync24Regular />} value="sync">同步</Tab>
                    <Tab icon={<PuzzlePiece24Regular />} value="captcha">验证码</Tab>
                    <Tab icon={<Database24Regular />} value="data">数据</Tab>
                    <Tab icon={<Tag24Regular />} value="classification">分类规则</Tab>
                    <Tab icon={<ArrowDownload24Regular />} value="update">更新</Tab>
                    <Tab icon={<Bug24Regular />} value="debug">调试</Tab>
                  </TabList>
                </div>
              </SectionEnterMotion>

              {/* Right Content */}
              <div style={{ paddingLeft: 16, borderLeft: '1px solid var(--colorNeutralStroke2)' }}>
                {message && (
                  <MessageBar intent={message.includes('失败') ? 'error' : 'success'} style={{ marginBottom: 12 }}>
                    <MessageBarBody>{message}</MessageBarBody>
                  </MessageBar>
                )}
                <PageEnterMotion key={selectedTab}>
                  <div>{renderContent()}</div>
                </PageEnterMotion>
              </div>
            </div>
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" onClick={() => setShowSettingsDialog(false)}>
              关闭
            </Button>
            <Button appearance="secondary" onClick={handleApply} disabled={saving}>
              {saving ? '应用中...' : '应用'}
            </Button>
            <Button appearance="primary" onClick={handleSaveAndClose} disabled={saving}>
              {saving ? '保存中...' : '保存'}
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
};
