import React, { useState } from 'react';
import {
  Dialog,
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
} from '@fluentui/react-components';
import { ShieldTask24Regular, ArrowCounterclockwise24Regular } from '@fluentui/react-icons';
import { useAppStore } from '../../stores/appStore';
import type { CaptchaMode, CaptchaTestResult } from '../../types';
import * as tauri from '../../services/tauri';

export const CaptchaTestDialog: React.FC = () => {
  const showCaptchaTestDialog = useAppStore((s) => s.showCaptchaTestDialog);
  const setShowCaptchaTestDialog = useAppStore((s) => s.setShowCaptchaTestDialog);
  const config = useAppStore((s) => s.config);

  const [mode, setMode] = useState<CaptchaMode>('manual');
  const [captchaImage, setCaptchaImage] = useState<string>('');
  const [testing, setTesting] = useState(false);
  const [batchTesting, setBatchTesting] = useState(false);
  const [testResults, setTestResults] = useState<CaptchaTestResult[]>([]);
  const [manualInput, setManualInput] = useState('');

  const normalizeCaptchaSrc = (value: string) =>
    value.startsWith('data:') ? value : `data:image/png;base64,${value}`;

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

  const handleTest = async () => {
    setTesting(true);
    try {
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
    } catch (e) {
      setTestResults((prev) => [
        {
          id: Date.now(),
          success: false,
          expression: '',
          answer: '',
          duration_ms: 0,
          mode,
          error: String(e),
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
      const results = await tauri.batch_test_captcha(mode, 10);
      setTestResults((prev) => [...results, ...prev]);
    } catch (e) {
      console.error('Batch test failed:', e);
    } finally {
      setBatchTesting(false);
    }
  };

  const modeLabel = mode === 'manual' ? '手动输入' : mode === 'remote_ocr' ? '远程OCR(旧)' : mode === 'remote_ocr_http' ? '远程OCR(RESTful)' : '本地ONNX';
  const imgSrc = captchaImage ? normalizeCaptchaSrc(captchaImage) : '';

  return (
    <Dialog open={showCaptchaTestDialog} onOpenChange={(_, data) => !data.open && setShowCaptchaTestDialog(false)}>
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
                    >
                      {mode === 'manual' ? '刷新并锁定当前验证码' : '刷新验证码'}
                    </Button>
                    <Button appearance="primary" onClick={handleTest} disabled={testing}>
                      {testing ? <Spinner size="tiny" /> : '开始测试'}
                    </Button>
                    <Button
                      appearance="secondary"
                      onClick={handleBatchTest}
                      disabled={batchTesting || mode === 'manual'}
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
                    background: 'linear-gradient(180deg, var(--colorNeutralBackground2), var(--colorNeutralBackground1))',
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
                        {mode === 'manual'
                          ? '先点击“刷新并锁定当前验证码”'
                          : '点击“刷新验证码”获取测试图片'}
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
                              <Badge
                                appearance="filled"
                                color={result.success ? 'success' : 'danger'}
                                size="small"
                              >
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
            <Button appearance="secondary" onClick={() => setShowCaptchaTestDialog(false)}>
              关闭
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
};
