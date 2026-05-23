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

  const [mode, setMode] = useState<CaptchaMode>('manual');
  const [captchaImage, setCaptchaImage] = useState<string>('');
  const [testing, setTesting] = useState(false);
  const [batchTesting, setBatchTesting] = useState(false);
  const [testResults, setTestResults] = useState<CaptchaTestResult[]>([]);
  const [manualInput, setManualInput] = useState('');

  const handleRefreshCaptcha = async () => {
    try {
      const image = await tauri.get_captcha_image();
      // Handle both plain base64 and pre-prefixed base64
      const normalized = image.startsWith('data:') ? image : `data:image/png;base64,${image}`;
      setCaptchaImage(normalized);
    } catch {
      setCaptchaImage('');
    }
  };

  const handleTest = async () => {
    setTesting(true);
    try {
      const result = await tauri.test_captcha(mode, mode === 'manual' ? manualInput : undefined);
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

  const modeLabel = mode === 'manual' ? '手动输入' : mode === 'remote_ocr' ? '远程OCR' : '本地ONNX';

  // Normalize captcha image src
  const imgSrc = captchaImage.startsWith('data:') ? captchaImage : `data:image/png;base64,${captchaImage}`;

  return (
    <Dialog open={showCaptchaTestDialog} onOpenChange={(_, data) => !data.open && setShowCaptchaTestDialog(false)}>
      <DialogSurface style={{ maxWidth: 550 }}>
        <DialogBody>
          <DialogTitle>
            <ShieldTask24Regular style={{ marginRight: 8 }} />
            验证码测试
          </DialogTitle>
          <DialogContent>
            <div style={{ display: 'grid', gap: 12 }}>
              <div>
                <Label>识别模式</Label>
                <Dropdown
                  value={modeLabel}
                  selectedOptions={[mode]}
                  onOptionSelect={(_, data) => setMode(data.optionValue as CaptchaMode)}
                  style={{ width: '100%' }}
                >
                  <Option value="manual">手动输入</Option>
                  <Option value="remote_ocr">远程OCR</Option>
                  <Option value="local_onnx">本地ONNX</Option>
                </Dropdown>
              </div>

              {/* Captcha Image Display */}
              <div
                style={{
                  border: '1px solid var(--colorNeutralStroke2)',
                  borderRadius: 4,
                  padding: 16,
                  textAlign: 'center',
                  minHeight: 80,
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  flexDirection: 'column',
                }}
              >
                {captchaImage ? (
                  <img
                    src={imgSrc}
                    alt="验证码"
                    style={{ maxWidth: 150, height: 50 }}
                  />
                ) : (
                  <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
                    点击"刷新验证码"获取图片
                  </Text>
                )}
              </div>

              <Button
                appearance="secondary"
                icon={<ArrowCounterclockwise24Regular />}
                onClick={handleRefreshCaptcha}
              >
                刷新验证码
              </Button>

              {mode === 'manual' && (
                <div>
                  <Label>手动输入验证码</Label>
                  <Input
                    value={manualInput}
                    onChange={(e) => setManualInput(e.currentTarget.value)}
                    placeholder="输入验证码答案"
                    style={{ width: '100%' }}
                  />
                </div>
              )}

              <div style={{ display: 'flex', gap: 8 }}>
                <Button appearance="primary" onClick={handleTest} disabled={testing}>
                  {testing ? <Spinner size="tiny" /> : '测试识别'}
                </Button>
                <Button appearance="secondary" onClick={handleBatchTest} disabled={batchTesting}>
                  {batchTesting ? <Spinner size="tiny" /> : '批量测试(10次)'}
                </Button>
              </div>

              {/* Test Results */}
              {testResults.length > 0 && (
                <>
                  <Label>测试历史</Label>
                  <Table style={{ maxHeight: 200, overflow: 'auto' }}>
                    <TableHeader>
                      <TableRow>
                        <TableHeaderCell>#</TableHeaderCell>
                        <TableHeaderCell>结果</TableHeaderCell>
                        <TableHeaderCell>表达式</TableHeaderCell>
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
                          <TableCell>{result.duration_ms}ms</TableCell>
                          <TableCell>
                            {result.mode === 'manual'
                              ? '手动'
                              : result.mode === 'remote_ocr'
                                ? '远程OCR'
                                : '本地ONNX'}
                          </TableCell>
                        </TableRow>
                      ))}
                    </TableBody>
                  </Table>
                </>
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
