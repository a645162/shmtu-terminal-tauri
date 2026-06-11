import React, { useEffect, useState, useCallback } from 'react';
import {
  Dialog,
  DialogTrigger,
  DialogSurface,
  DialogTitle,
  DialogBody,
  DialogContent,
  DialogActions,
  Button,
  Dropdown,
  Option,
  Input,
  Text,
  Label,
  Spinner,
  Badge,
  Table,
  TableHeader,
  TableRow,
  TableCell,
  TableBody,
  TableHeaderCell,
  MessageBar,
  MessageBarBody,
  Tooltip,
  Field,
} from '@fluentui/react-components';
import { listen } from '@tauri-apps/api/event';
import { ShieldTask24Regular, ArrowCounterclockwise24Regular, Settings24Regular, ArrowDownload24Regular } from '@fluentui/react-icons';
import { useAppStore } from '../../stores/appStore';
import type {
  CaptchaMode,
  CaptchaTestResult,
  LocalOcrModelDownloadProgress,
} from '../../types';
import * as tauri from '../../services/tauri';
import { LocalOcrModelDownloadDialog } from './LocalOcrModelDownloadDialog';

export const CaptchaTestDialog: React.FC = () => {
  const showCaptchaTestDialog = useAppStore((s) => s.showCaptchaTestDialog);
  const setShowCaptchaTestDialog = useAppStore((s) => s.setShowCaptchaTestDialog);
  const openSettingsDialog = useAppStore((s) => s.openSettingsDialog);
  const config = useAppStore((s) => s.config);

  const [mode, setMode] = useState<CaptchaMode>(config?.captcha.mode ?? 'manual');
  const [captchaImage, setCaptchaImage] = useState<string>('');
  const [testing, setTesting] = useState(false);
  const [batchTesting, setBatchTesting] = useState(false);
  const [testResults, setTestResults] = useState<CaptchaTestResult[]>([]);
  const [manualInput, setManualInput] = useState('');
  const [localModelProgress, setLocalModelProgress] = useState<LocalOcrModelDownloadProgress | null>(null);
  const [showLocalModelDownloadDialog, setShowLocalModelDownloadDialog] = useState(false);
  const [localModelCancelling, setLocalModelCancelling] = useState(false);
  const [localModelMessage, setLocalModelMessage] = useState('');
  const [showLocalModelRecoveryDialog, setShowLocalModelRecoveryDialog] = useState(false);
  const [localModelRecoveryError, setLocalModelRecoveryError] = useState('');
  const [recoveringLocalModels, setRecoveringLocalModels] = useState(false);

  // ---- OCR model settings ----
  const [ocrModelVersion, setOcrModelVersion] = useState<'v1' | 'v2'>('v2');
  const [ocrV2Config, setOcrV2Config] = useState<tauri.OcrV2Config | null>(null);
  const [showAdvancedModelDialog, setShowAdvancedModelDialog] = useState(false);
  const [quickDownloading, setQuickDownloading] = useState(false);
  const [quickDownloadResult, setQuickDownloadResult] = useState('');

  const loadOcrConfig = useCallback(async () => {
    try {
      const version = await tauri.get_ocr_model_version();
      setOcrModelVersion(version);
      if (version === 'v2') {
        const cfg = await tauri.get_ocr_v2_config();
        setOcrV2Config(cfg);
      }
    } catch {
      // Ignore errors — settings may not apply yet.
    }
  }, []);

  useEffect(() => {
    if (!showCaptchaTestDialog) {
      return;
    }
    void loadOcrConfig();
  }, [showCaptchaTestDialog, loadOcrConfig]);

  const handleVersionSwitch = async (version: 'v1' | 'v2') => {
    try {
      await tauri.set_ocr_model_version(version);
      setOcrModelVersion(version);
      if (version === 'v2') {
        const cfg = await tauri.get_ocr_v2_config();
        setOcrV2Config(cfg);
      } else {
        setOcrV2Config(null);
      }
      setQuickDownloadResult('');
    } catch (error) {
      setQuickDownloadResult(`切换失败: ${String(error)}`);
    }
  };

  const handleQuickDownload = async () => {
    setQuickDownloading(true);
    setQuickDownloadResult('');
    try {
      // Force v2 if needed for model downloads
      if (ocrModelVersion !== 'v2') {
        await tauri.set_ocr_model_version('v2');
        setOcrModelVersion('v2');
      }
      // Set defaults: latest tag, mobilenet_v3_small, fp16
      await tauri.ocr_v2_resolve_latest_tag();
      await tauri.set_ocr_v2_backbone('mobilenet_v3_small');
      await tauri.set_ocr_v2_precision('fp16');
      const cfg = await tauri.get_ocr_v2_config();
      setOcrV2Config(cfg);
      // Download if not already ready.
      const status = await tauri.get_local_ocr_model_status();
      if (!status.ready) {
        await tauri.ensure_local_ocr_models();
      }
      setQuickDownloadResult('默认模型 (mobilenet_v3_small + fp16) 已就绪');
    } catch (error) {
      setQuickDownloadResult(`下载失败: ${String(error)}`);
    } finally {
      setQuickDownloading(false);
    }
  };

  const normalizeCaptchaSrc = (value: string) =>
    value.startsWith('data:') ? value : `data:image/png;base64,${value}`;

  const localModelDownloadBusy =
    showLocalModelDownloadDialog &&
    (localModelProgress?.phase === 'checking' || localModelProgress?.phase === 'downloading');

  const isRecoverableLocalModelError = (message: string) =>
    mode === 'local_onnx' &&
    !localModelDownloadBusy &&
    (message.includes('加载ONNX模型失败') ||
      message.includes('加载数字模型失败') ||
      message.includes('加载运算符模型失败') ||
      message.includes('加载等号模型失败') ||
      message.includes('本地ONNX识别失败: ONNX推理失败') ||
      message.includes('本地ONNX识别失败: 解析图片字节失败'));

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;

    listen<LocalOcrModelDownloadProgress>('local-ocr-model-download', (event) => {
      if (disposed) {
        return;
      }

      const progress = event.payload;
      setLocalModelProgress(progress);

      if (progress.phase === 'checking' || progress.phase === 'downloading') {
        setShowLocalModelDownloadDialog(true);
      }

      if (progress.phase === 'completed') {
        setLocalModelMessage(progress.message);
        setLocalModelCancelling(false);
      }

      if (progress.phase === 'cancelled' || progress.phase === 'error') {
        setLocalModelMessage(progress.message);
        setLocalModelCancelling(false);
      }
    })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(console.error);

    return () => {
      disposed = true;
      if (unlisten) {
        void unlisten();
      }
    };
  }, []);

  useEffect(() => {
    if (!showCaptchaTestDialog || !config?.captcha.mode) {
      return;
    }
    setMode(config.captcha.mode);
  }, [config?.captcha.mode, showCaptchaTestDialog]);

  const ensureLocalModelsReady = async () => {
    const status = await tauri.get_local_ocr_model_status();
    if (status.ready) {
      setLocalModelMessage('');
      return true;
    }

    setLocalModelMessage('');
    setLocalModelCancelling(false);
    setShowLocalModelDownloadDialog(true);
    setLocalModelProgress({
      phase: 'checking',
      model_dir: status.model_dir,
      total_files: status.total_files,
      completed_files: status.existing_files,
      current_file_progress: 0,
      overall_progress: status.existing_files / Math.max(status.total_files, 1),
      message: `检测到缺少 ${status.missing_files.length} 个 ${status.model_version || ''} 模型文件，准备下载...`,
    });

    try {
      await tauri.ensure_local_ocr_models();
      setShowLocalModelDownloadDialog(false);
      return true;
    } catch (error) {
      const message = String(error);
      setShowLocalModelDownloadDialog(false);
      if (message.includes('已取消')) {
        return false;
      }
      setLocalModelMessage(message);
      return false;
    }
  };

  const handleRefreshCaptcha = async () => {
    try {
      if (mode === 'manual') {
        const challenge = await tauri.get_captcha_with_execution();
        setCaptchaImage(normalizeCaptchaSrc(challenge.captcha_image));
        setManualInput('');
        return;
      }

      const image = await tauri.get_captcha_image();
      setCaptchaImage(normalizeCaptchaSrc(image));
    } catch {
      setCaptchaImage('');
    }
  };

  const runSingleTest = async () => {
    const result = await tauri.test_captcha(mode, mode === 'manual' ? manualInput : undefined);
    if (mode === 'manual') {
      if (result.captcha_image) {
        setCaptchaImage(normalizeCaptchaSrc(result.captcha_image));
        setManualInput('');
      } else if (result.success) {
        setCaptchaImage('');
        setManualInput('');
      }
    }
    setTestResults((prev) => [result, ...prev]);
    return result;
  };

  const handleRecoverLocalModels = async () => {
    setRecoveringLocalModels(true);
    try {
      await tauri.delete_local_ocr_models();
      const ready = await ensureLocalModelsReady();
      if (!ready) {
        return;
      }
      setShowLocalModelRecoveryDialog(false);
      setLocalModelRecoveryError('');
      await runSingleTest();
    } catch (error) {
      setLocalModelMessage(String(error));
    } finally {
      setRecoveringLocalModels(false);
    }
  };

  const handleTest = async () => {
    setTesting(true);
    try {
      if (mode === 'local_onnx') {
        const ready = await ensureLocalModelsReady();
        if (!ready) {
          return;
        }
      }
      await runSingleTest();
    } catch (e) {
      const errorMessage = String(e);
      if (isRecoverableLocalModelError(errorMessage)) {
        setLocalModelRecoveryError(errorMessage);
        setShowLocalModelRecoveryDialog(true);
        return;
      }
      setTestResults((prev) => [
        {
          id: Date.now(),
          success: false,
          expression: '',
          answer: '',
          duration_ms: 0,
          mode,
          error: errorMessage,
        },
        ...prev,
      ]);
    } finally {
      setTesting(false);
    }
  };

  const handleBatchTest = async () => {
    setBatchTesting(true);
    try {
      if (mode === 'local_onnx') {
        const ready = await ensureLocalModelsReady();
        if (!ready) {
          return;
        }
      }
      for (let index = 0; index < 10; index += 1) {
        try {
          const result = await tauri.test_captcha(mode);
          setTestResults((prev) => [{ ...result, id: Date.now() + index }, ...prev]);
        } catch (error) {
          const errorMessage = String(error);
          setTestResults((prev) => [
            {
              id: Date.now() + index,
              success: false,
              expression: '',
              answer: '',
              duration_ms: 0,
              mode,
              error: `第 ${index + 1} 次失败: ${errorMessage}`,
            },
            ...prev,
          ]);
          break;
        }
      }
    } catch (e) {
      console.error('Batch test failed:', e);
    } finally {
      setBatchTesting(false);
    }
  };

  const modeLabel =
    mode === 'manual'
      ? '手动输入'
      : mode === 'remote_ocr'
        ? '远程OCR(旧)'
        : mode === 'remote_ocr_http'
          ? '远程OCR(RESTful)'
          : '本地ONNX';
  const imgSrc = captchaImage ? normalizeCaptchaSrc(captchaImage) : '';
  const totalTests = testResults.length;
  const successCount = testResults.filter((result) => result.success).length;
  const failureCount = totalTests - successCount;
  const accuracy = totalTests > 0 ? (successCount / totalTests) * 100 : 0;
  const averageDuration =
    totalTests > 0
      ? testResults.reduce((sum, result) => sum + result.duration_ms, 0) / totalTests
      : 0;

  return (
    <>
      <Dialog
        open={showCaptchaTestDialog}
        onOpenChange={(_, data) => {
          if (!data.open && !localModelDownloadBusy) {
            setShowCaptchaTestDialog(false);
          }
        }}
      >
        <DialogSurface style={{ width: 'min(92vw, 860px)', maxWidth: 860 }}>
          <DialogBody>
            <DialogTitle>
              <ShieldTask24Regular style={{ marginRight: 8 }} />
              验证码测试
            </DialogTitle>
            <DialogContent>
              <div style={{ display: 'grid', gap: 16 }}>
              <div
                style={{
                  display: 'grid',
                  gridTemplateColumns: 'minmax(0, 1.15fr) minmax(280px, 0.85fr)',
                  gap: 16,
                  alignItems: 'start',
                }}
              >
                <div
                  style={{
                    display: 'grid',
                    gap: 14,
                    padding: 16,
                    borderRadius: 10,
                    border: '1px solid var(--colorNeutralStroke2)',
                    background: 'var(--colorNeutralBackground1)',
                  }}
                >
                  <div>
                    <Label>识别模式</Label>
                    <Dropdown
                      value={modeLabel}
                      selectedOptions={[mode]}
                      onOptionSelect={(_, data) => setMode(data.optionValue as CaptchaMode)}
                      style={{ width: '100%', marginTop: 6 }}
                    >
                      <Option value="manual">手动输入</Option>
                      <Option value="remote_ocr">远程OCR(旧)</Option>
                      <Option value="remote_ocr_http">远程OCR(RESTful)</Option>
                      <Option value="local_onnx">本地ONNX</Option>
                    </Dropdown>
                    <div style={{ display: 'flex', justifyContent: 'space-between', gap: 12, marginTop: 8, alignItems: 'center' }}>
                      <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
                        默认模式跟随“设置 → 验证码”
                      </Text>
                      <Button
                        appearance="subtle"
                        size="small"
                        onClick={() => openSettingsDialog('captcha')}
                        disabled={localModelDownloadBusy}
                      >
                        打开验证码设置
                      </Button>
                    </div>
                  </div>

                  {mode === 'manual' ? (
                    <MessageBar layout="multiline">
                      <MessageBarBody>
                        手动模式会校验你当前看到的这张验证码。提交一个固定错误账号密码时，如果返回“密码错误”，说明验证码正确；如果返回“验证码错误”，说明这次输入错了。
                      </MessageBarBody>
                    </MessageBar>
                  ) : (
                    (mode === 'remote_ocr' || mode === 'remote_ocr_http') && config?.captcha && (
                      <MessageBar>
                        <MessageBarBody>
                          当前 OCR 服务器：
                          {mode === 'remote_ocr'
                            ? ` TCP (${config.captcha.remote_ocr_host}:${config.captcha.remote_ocr_port})`
                            : ` RESTful (${config.captcha.remote_ocr_http_url})`}
                        </MessageBarBody>
                      </MessageBar>
                    )
                  )}

                  {mode === 'local_onnx' && localModelMessage && (
                    <MessageBar>
                      <MessageBarBody>{localModelMessage}</MessageBarBody>
                    </MessageBar>
                  )}

                  {mode === 'local_onnx' && (
                    <>
                      {/* OCR Model Configuration Section */}
                      <div
                        style={{
                          padding: 14,
                          borderRadius: 8,
                          border: '1px solid var(--colorNeutralStroke2)',
                          background: 'var(--colorNeutralBackground2)',
                          display: 'grid',
                          gap: 12,
                        }}
                      >
                        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                          <Text weight="semibold">OCR 模型设置</Text>
                          <Field label={undefined} style={{ marginBottom: 0 }}>
                            <div style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
                              <Tooltip content="v1: 3 模型 ResNet (legacy)；v2: 单模型 MobileNetV3 Tri-Slot Decoder (默认)" relationship="label">
                                <Text size={200}>版本</Text>
                              </Tooltip>
                              <Dropdown
                                value={ocrModelVersion}
                                selectedOptions={[ocrModelVersion]}
                                onOptionSelect={(_, data) => void handleVersionSwitch(data.optionValue as 'v1' | 'v2')}
                                disabled={localModelDownloadBusy || showLocalModelDownloadDialog}
                                style={{ minWidth: 72, fontSize: 12 }}
                                size="small"
                              >
                                <Option value="v2" text="v2">v2 (默认)</Option>
                                <Option value="v1" text="v1">v1</Option>
                              </Dropdown>
                            </div>
                          </Field>
                        </div>

                        {ocrModelVersion === 'v2' && ocrV2Config && (
                          <div style={{ display: 'grid', gap: 6 }}>
                            <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
                              当前 v2 配置: tag=<code>{ocrV2Config.tag}</code> · backbone=<code>{ocrV2Config.backbone}</code> · precision=<code>{ocrV2Config.precision}</code>
                            </Text>
                          </div>
                        )}

                        <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
                          <Button
                            appearance="primary"
                            size="small"
                            icon={<ArrowDownload24Regular />}
                            onClick={() => void handleQuickDownload()}
                            disabled={quickDownloading || localModelDownloadBusy}
                          >
                            {quickDownloading ? <Spinner size="tiny" /> : '一键下载默认模型'}
                          </Button>
                          <Button
                            appearance="subtle"
                            size="small"
                            icon={<Settings24Regular />}
                            onClick={() => setShowAdvancedModelDialog(true)}
                            disabled={localModelDownloadBusy}
                          >
                            高级设置
                          </Button>
                        </div>

                        {quickDownloadResult && (
                          <Text size={200} style={{ color: quickDownloadResult.includes('失败') ? 'var(--colorPaletteRedForeground1)' : 'var(--colorPaletteGreenForeground1)' }}>
                            {quickDownloadResult}
                          </Text>
                        )}
                      </div>
                    </>
                  )}

                  {mode === 'manual' && (
                    <div>
                      <Label>手动输入验证码</Label>
                      <Input
                        value={manualInput}
                        onChange={(e) => setManualInput(e.currentTarget.value)}
                        placeholder="输入当前图片中的答案"
                        style={{ width: '100%', marginTop: 6 }}
                      />
                    </div>
                  )}

                  <div style={{ display: 'flex', gap: 10, flexWrap: 'wrap' }}>
                    <Button
                      appearance="secondary"
                      icon={<ArrowCounterclockwise24Regular />}
                      onClick={handleRefreshCaptcha}
                      disabled={localModelDownloadBusy}
                    >
                      {mode === 'manual' ? '刷新并锁定当前验证码' : '刷新验证码'}
                    </Button>
                    <Button appearance="primary" onClick={handleTest} disabled={testing || localModelDownloadBusy}>
                      {testing ? <Spinner size="tiny" /> : '开始测试'}
                    </Button>
                    <Button
                      appearance="secondary"
                      onClick={handleBatchTest}
                      disabled={batchTesting || mode === 'manual' || localModelDownloadBusy}
                    >
                      {batchTesting ? <Spinner size="tiny" /> : '批量测试(10次)'}
                    </Button>
                  </div>
                </div>

                <div
                  style={{
                    display: 'grid',
                    gap: 10,
                    padding: 16,
                    borderRadius: 10,
                    border: '1px solid var(--colorNeutralStroke2)',
                    background:
                      'linear-gradient(180deg, var(--colorNeutralBackground2), var(--colorNeutralBackground1))',
                  }}
                >
                  <Text weight="semibold">当前验证码</Text>
                  <div
                    style={{
                      border: '1px dashed var(--colorNeutralStroke2)',
                      borderRadius: 8,
                      minHeight: 180,
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'center',
                      background: 'var(--colorNeutralBackground1)',
                      padding: 16,
                    }}
                  >
                    {captchaImage ? (
                      <img
                        src={imgSrc}
                        alt="验证码"
                        style={{ width: '100%', maxWidth: 240, objectFit: 'contain' }}
                      />
                    ) : (
                      <Text size={300} style={{ color: 'var(--colorNeutralForeground3)', textAlign: 'center' }}>
                        {mode === 'manual' ? '先点击“刷新并锁定当前验证码”' : '点击“刷新验证码”获取测试图片'}
                      </Text>
                    )}
                  </div>
                  <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
                    手动模式必须先刷新图片，再输入这张图上的结果。验证码输错后，界面会自动切换到下一张图。
                  </Text>
                </div>
              </div>

              {testResults.length > 0 && (
                <div
                  style={{
                    display: 'grid',
                    gap: 10,
                    padding: 16,
                    borderRadius: 10,
                    border: '1px solid var(--colorNeutralStroke2)',
                    background: 'var(--colorNeutralBackground1)',
                  }}
                >
                  <Label>测试历史</Label>
                  <div
                    style={{
                      display: 'grid',
                      gridTemplateColumns: 'repeat(auto-fit, minmax(120px, 1fr))',
                      gap: 10,
                    }}
                  >
                    <div
                      style={{
                        padding: 12,
                        borderRadius: 8,
                        background: 'var(--colorNeutralBackground2)',
                        border: '1px solid var(--colorNeutralStroke2)',
                      }}
                    >
                      <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>总次数</Text>
                      <Text weight="semibold" size={400}>{totalTests}</Text>
                    </div>
                    <div
                      style={{
                        padding: 12,
                        borderRadius: 8,
                        background: 'var(--colorPaletteGreenBackground1)',
                        border: '1px solid var(--colorPaletteGreenBorder2)',
                      }}
                    >
                      <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>通过</Text>
                      <Text weight="semibold" size={400}>{successCount}</Text>
                    </div>
                    <div
                      style={{
                        padding: 12,
                        borderRadius: 8,
                        background: 'var(--colorPaletteRedBackground1)',
                        border: '1px solid var(--colorPaletteRedBorder2)',
                      }}
                    >
                      <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>失败</Text>
                      <Text weight="semibold" size={400}>{failureCount}</Text>
                    </div>
                    <div
                      style={{
                        padding: 12,
                        borderRadius: 8,
                        background: 'var(--colorNeutralBackground2)',
                        border: '1px solid var(--colorNeutralStroke2)',
                      }}
                    >
                      <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>正确率</Text>
                      <Text weight="semibold" size={400}>{accuracy.toFixed(1)}%</Text>
                    </div>
                    <div
                      style={{
                        padding: 12,
                        borderRadius: 8,
                        background: 'var(--colorNeutralBackground2)',
                        border: '1px solid var(--colorNeutralStroke2)',
                      }}
                    >
                      <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>平均耗时</Text>
                      <Text weight="semibold" size={400}>{averageDuration.toFixed(0)}ms</Text>
                    </div>
                  </div>
                  <div style={{ maxHeight: 240, overflow: 'auto' }}>
                    <Table>
                      <TableHeader>
                        <TableRow>
                          <TableHeaderCell>#</TableHeaderCell>
                          <TableHeaderCell>结果</TableHeaderCell>
                          <TableHeaderCell>识别/错误</TableHeaderCell>
                          <TableHeaderCell>校验</TableHeaderCell>
                          <TableHeaderCell>耗时</TableHeaderCell>
                          <TableHeaderCell>模式</TableHeaderCell>
                        </TableRow>
                      </TableHeader>
                      <TableBody>
                        {testResults.map((result, idx) => (
                          <TableRow key={result.id}>
                            <TableCell>{idx + 1}</TableCell>
                            <TableCell>
                              <Badge appearance="filled" color={result.success ? 'success' : 'danger'} size="small">
                                {result.success ? '通过' : '失败'}
                              </Badge>
                            </TableCell>
                            <TableCell>{result.expression || result.error || '—'}</TableCell>
                            <TableCell>{result.verification || '—'}</TableCell>
                            <TableCell>{result.duration_ms}ms</TableCell>
                            <TableCell>
                              {result.mode === 'manual'
                                ? '手动'
                                : result.mode === 'remote_ocr'
                                  ? '远程OCR(旧)'
                                  : result.mode === 'remote_ocr_http'
                                    ? '远程OCR(RESTful)'
                                    : '本地ONNX'}
                            </TableCell>
                          </TableRow>
                        ))}
                      </TableBody>
                    </Table>
                  </div>
                </div>
              )}
              </div>
            </DialogContent>
            <DialogActions>
              <Button
                appearance="secondary"
                onClick={() => setShowCaptchaTestDialog(false)}
                disabled={localModelDownloadBusy}
              >
                关闭
              </Button>
            </DialogActions>
          </DialogBody>
        </DialogSurface>
      </Dialog>
      <LocalOcrModelDownloadDialog
        open={showLocalModelDownloadDialog}
        progress={localModelProgress}
        cancelling={localModelCancelling}
        onCancel={() => {
          setLocalModelCancelling(true);
          tauri.cancel_local_ocr_model_download().catch((error) => {
            setLocalModelCancelling(false);
            setLocalModelMessage(String(error));
          });
        }}
      />
      <LocalOcrModelDownloadDialog
        open={showAdvancedModelDialog}
        progress={null}
        cancelling={false}
        advanced
        onCancel={async () => {
          setShowAdvancedModelDialog(false);
          // Reload config after user finishes configuring via advanced dialog.
          await loadOcrConfig();
        }}
      />
      <Dialog open={showLocalModelRecoveryDialog}>
        <DialogSurface style={{ width: 'min(92vw, 520px)' }}>
          <DialogBody>
            <DialogTitle>本地 OCR 模型加载失败</DialogTitle>
            <DialogContent>
              <div style={{ display: 'grid', gap: 12 }}>
                <Text>
                  检测到模型文件已存在，但加载失败。是否删除现有模型文件后重新下载，再重试本次识别？
                </Text>
                {localModelRecoveryError && (
                  <MessageBar intent="warning">
                    <MessageBarBody>{localModelRecoveryError}</MessageBarBody>
                  </MessageBar>
                )}
              </div>
            </DialogContent>
            <DialogActions>
              <DialogTrigger disableButtonEnhancement>
                <Button
                  appearance="secondary"
                  onClick={() => {
                    if (!recoveringLocalModels) {
                      setShowLocalModelRecoveryDialog(false);
                    }
                  }}
                  disabled={recoveringLocalModels}
                >
                  取消
                </Button>
              </DialogTrigger>
              <Button appearance="primary" onClick={handleRecoverLocalModels} disabled={recoveringLocalModels}>
                {recoveringLocalModels ? <Spinner size="tiny" /> : '删除并重新下载'}
              </Button>
            </DialogActions>
          </DialogBody>
        </DialogSurface>
      </Dialog>
    </>
  );
};
