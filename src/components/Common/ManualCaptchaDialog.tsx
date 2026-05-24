import React, { useState } from 'react';
import {
  Dialog,
  DialogSurface,
  DialogTitle,
  DialogBody,
  DialogContent,
  DialogActions,
  Button,
  Input,
  Text,
} from '@fluentui/react-components';
import { Shield24Regular } from '@fluentui/react-icons';
import { useAppStore } from '../../stores/appStore';
import * as tauri from '../../services/tauri';

interface ManualCaptchaDialogProps {
  captchaImage: string;
  execution: string;
  onSuccess: () => void;
  onCancel: () => void;
}

export const ManualCaptchaDialog: React.FC<ManualCaptchaDialogProps> = ({
  captchaImage,
  execution,
  onSuccess,
  onCancel,
}) => {
  const [captchaCode, setCaptchaCode] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const currentIdentity = useAppStore((s) => s.currentIdentity);
  const loadBills = useAppStore((s) => s.loadBills);
  const setCaptchaForManualLogin = useAppStore((s) => s.setCaptchaForManualLogin);

  const handleSubmit = async () => {
    if (!currentIdentity || !captchaCode.trim()) return;

    setIsLoading(true);
    setError(null);

    try {
      const result = await tauri.sync_with_captcha(
        currentIdentity.id,
        captchaCode.trim(),
        execution
      );

      if (result.status === 'completed') {
        loadBills();
        onSuccess();
      } else if (result.status === 'captcha_required' && result.captcha_image && result.execution) {
        // 更新验证码图片并重新输入
        setCaptchaForManualLogin(result.captcha_image, result.execution);
        setCaptchaCode(''); // 清空输入框
        setError(result.error ?? '验证码错误，请重新输入');
      } else {
        setError('验证码错误，请重试');
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setIsLoading(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && captchaCode.trim()) {
      handleSubmit();
    }
  };

  return (
    <Dialog open>
      <DialogSurface>
        <DialogBody>
          <DialogTitle>
            <Shield24Regular style={{ marginRight: 8 }} />
            请输入验证码
          </DialogTitle>
          <DialogContent>
            <Text block style={{ marginBottom: 12 }}>
              请查看下方验证码图片并输入识别结果：
            </Text>

            <div
              style={{
                display: 'flex',
                justifyContent: 'center',
                marginBottom: 16,
                padding: 16,
                backgroundColor: 'var(--colorNeutralBackground6)',
                borderRadius: 8,
              }}
            >
              <img
                src={`data:image/png;base64,${captchaImage}`}
                alt="验证码"
                style={{
                  maxWidth: '100%',
                  height: 'auto',
                  borderRadius: 4,
                }}
              />
            </div>

            <Input
              placeholder="请输入验证码"
              value={captchaCode}
              onChange={(e) => setCaptchaCode(e.currentTarget.value)}
              onKeyDown={handleKeyDown}
              style={{ width: '100%', marginBottom: 8 }}
              disabled={isLoading}
              autoFocus
            />

            {error && (
              <Text
                size={200}
                style={{ color: 'var(--colorPaletteRedForeground3)', display: 'block' }}
              >
                {error}
              </Text>
            )}
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" onClick={onCancel} disabled={isLoading}>
              取消
            </Button>
            <Button
              appearance="primary"
              onClick={handleSubmit}
              disabled={!captchaCode.trim() || isLoading}
            >
              {isLoading ? '提交中...' : '确认'}
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
};
