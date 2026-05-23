import React, { useState } from 'react';
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
} from '@fluentui/react-components';
import { useAppStore } from '../../stores/appStore';
import type { CaptchaMode, AppTheme } from '../../types';
import * as tauri from '../../services/tauri';

type SettingsTab = 'security' | 'identity' | 'captcha' | 'sync' | 'data' | 'ui' | 'classification' | 'update';
type IdentityStartupMode = 'last_used' | 'configured_default';

export const SettingsDialog: React.FC = () => {
  const showSettingsDialog = useAppStore((s) => s.showSettingsDialog);
  const setShowSettingsDialog = useAppStore((s) => s.setShowSettingsDialog);
  const config = useAppStore((s) => s.config);
  const theme = useAppStore((s) => s.theme);
  const setTheme = useAppStore((s) => s.setTheme);
  const loadConfig = useAppStore((s) => s.loadConfig);
  const identities = useAppStore((s) => s.identities);

  const [selectedTab, setSelectedTab] = useState<SettingsTab>('security');
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
  const [ocrPort, setOcrPort] = useState(config?.captcha.remote_ocr_port?.toString() ?? '');
  const [ocrRetry, setOcrRetry] = useState(config?.captcha.ocr_retry_count ?? 3);

  // Sync settings
  const [maxPages, setMaxPages] = useState(config?.sync.max_pages ?? 100);
  const [earlyStop, setEarlyStop] = useState(config?.sync.early_stop_threshold ?? 5);
  const [autoMerge, setAutoMerge] = useState(config?.sync.auto_merge_after_sync ?? true);

  // Data settings
  const [dataDir, setDataDir] = useState(config?.data.data_directory ?? 'Data');
  const [snapshotKeep, setSnapshotKeep] = useState(config?.data.snapshot_keep_count ?? 10);
  const [rulesUpdateUrl, setRulesUpdateUrl] = useState(config?.classification.rules_update_url ?? '');
  const [rulesPath, setRulesPath] = useState(config?.classification.rules_path ?? '');
  const currentDefaultIdentity =
    identities.find((identity) => identity.id === config?.identity.default_identity_id) ?? null;

  const handleSave = async () => {
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
          onnx_model_path: config.captcha.onnx_model_path,
          ocr_retry_count: ocrRetry,
        },
        sync: {
          max_pages: maxPages,
          early_stop_threshold: earlyStop,
          auto_merge_after_sync: autoMerge,
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
        },
        ui: {
          theme,
          language: config.ui.language,
        },
      };

      await tauri.save_config(nextConfig);

      if (startupProtection && passwordChanged && protectionPassword.trim()) {
        await tauri.set_startup_password(protectionPassword.trim());
      }

      await loadConfig();
      setMessage('设置已保存');
    } catch (e) {
      setMessage('保存失败');
      console.error('Failed to save config:', e);
    } finally {
      setSaving(false);
    }
  };

  const renderContent = () => {
    switch (selectedTab) {
      case 'security':
        return (
          <div style={{ display: 'grid', gap: 16 }}>
            <Text weight="semibold" size={400}>安全设置</Text>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <div>
                <Text block>启动保护</Text>
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
                <Label>保护密码</Label>
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
              <Label>启动时优先加载</Label>
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
              <Label>识别模式</Label>
              <Dropdown
                value={captchaMode === 'manual' ? '手动输入' : captchaMode === 'remote_ocr' ? '远程OCR' : '本地ONNX'}
                selectedOptions={[captchaMode]}
                onOptionSelect={(_, data) => setCaptchaMode(data.optionValue as CaptchaMode)}
                style={{ width: '100%' }}
              >
                <Option value="manual">手动输入</Option>
                <Option value="remote_ocr">远程OCR</Option>
                <Option value="local_onnx">本地ONNX</Option>
              </Dropdown>
            </div>
            {captchaMode === 'remote_ocr' && (
              <>
                <div>
                  <Label>OCR服务器地址</Label>
                  <Input
                    value={ocrHost}
                    onChange={(e) => setOcrHost(e.currentTarget.value)}
                    placeholder="如: 192.168.1.100"
                    style={{ width: '100%' }}
                  />
                </div>
                <div>
                  <Label>OCR服务器端口</Label>
                  <Input
                    value={ocrPort}
                    onChange={(e) => setOcrPort(e.currentTarget.value)}
                    placeholder="如: 8888"
                    style={{ width: '100%' }}
                  />
                </div>
              </>
            )}
            {captchaMode !== 'manual' && (
              <div>
                <Label>OCR重试次数: {ocrRetry}</Label>
                <Slider
                  min={1}
                  max={10}
                  value={ocrRetry}
                  onChange={(_, data) => setOcrRetry(data.value)}
                />
              </div>
            )}
          </div>
        );

      case 'sync':
        return (
          <div style={{ display: 'grid', gap: 16 }}>
            <Text weight="semibold" size={400}>同步设置</Text>
            <div>
              <Label>默认同步页数上限: {maxPages}</Label>
              <Slider
                min={10}
                max={500}
                step={10}
                value={maxPages}
                onChange={(_, data) => setMaxPages(data.value)}
              />
            </div>
            <div>
              <Label>提前停止阈值: {earlyStop}</Label>
              <Slider
                min={1}
                max={20}
                value={earlyStop}
                onChange={(_, data) => setEarlyStop(data.value)}
              />
            </div>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <div>
                <Text block>同步后自动合并</Text>
                <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
                  自动将新条目追加到合并数据表
                </Text>
              </div>
              <Switch checked={autoMerge} onChange={(_, data) => setAutoMerge(data.checked)} />
            </div>
          </div>
        );

      case 'data':
        return (
          <div style={{ display: 'grid', gap: 16 }}>
            <Text weight="semibold" size={400}>数据设置</Text>
            <div>
              <Label>数据目录</Label>
              <Input
                value={dataDir}
                onChange={(e) => setDataDir(e.currentTarget.value)}
                style={{ width: '100%' }}
              />
            </div>
            <div>
              <Label>快照自动保留数: {snapshotKeep}</Label>
              <Slider
                min={1}
                max={50}
                value={snapshotKeep}
                onChange={(_, data) => setSnapshotKeep(data.value)}
              />
            </div>
          </div>
        );

      case 'ui':
        return (
          <div style={{ display: 'grid', gap: 16 }}>
            <Text weight="semibold" size={400}>界面设置</Text>
            <div>
              <Label>主题</Label>
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
          </div>
        );

      case 'classification':
        return (
          <div style={{ display: 'grid', gap: 16 }}>
            <Text weight="semibold" size={400}>分类规则设置</Text>
            <div>
              <Label>分类规则文件路径</Label>
              <Input
                value={rulesPath}
                onChange={(e) => setRulesPath(e.currentTarget.value)}
                placeholder="Data/classification_rules.toml"
                style={{ width: '100%' }}
              />
            </div>
            <div>
              <Label>规则更新源(GitHub)</Label>
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
              <Text>自动检查更新</Text>
              <Switch defaultChecked />
            </div>
            <div>
              <Label>检查间隔(小时)</Label>
              <Slider min={1} max={168} defaultValue={24} />
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
              <div>
                <TabList
                  vertical
                  selectedValue={selectedTab}
                  onTabSelect={(_, data) => setSelectedTab(data.value as SettingsTab)}
                >
                  <Tab value="security">安全</Tab>
                  <Tab value="identity">身份</Tab>
                  <Tab value="captcha">验证码</Tab>
                  <Tab value="sync">同步</Tab>
                  <Tab value="data">数据</Tab>
                  <Tab value="ui">界面</Tab>
                  <Tab value="classification">分类规则</Tab>
                  <Tab value="update">更新</Tab>
                </TabList>
              </div>

              {/* Right Content */}
              <div style={{ paddingLeft: 16, borderLeft: '1px solid var(--colorNeutralStroke2)' }}>
                {message && (
                  <MessageBar intent={message.includes('失败') ? 'error' : 'success'} style={{ marginBottom: 12 }}>
                    <MessageBarBody>{message}</MessageBarBody>
                  </MessageBar>
                )}
                {renderContent()}
              </div>
            </div>
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" onClick={() => setShowSettingsDialog(false)}>
              取消
            </Button>
            <Button appearance="primary" onClick={handleSave} disabled={saving}>
              {saving ? '保存中...' : '保存'}
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
};
