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
  const syncProgress = useAppStore((s) => s.syncProgress);
  const submitManualCaptcha = useAppStore((s) => s.submitManualCaptcha);

  const error =
    syncProgress?.status === 'captcha_required' ? syncProgress.error : null;

  const handleSubmit = async () => {
    if (!captchaCode.trim()) return;
    await submitManualCaptcha(captchaCode.trim(), execution);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && captchaCode.trim()) {
      void handleSubmit();
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
            <Button appearance="secondary" onClick={onCancel}>
              取消
            </Button>
            <Button
              appearance="primary"
              onClick={() => void handleSubmit()}
              disabled={!captchaCode.trim()}
            >
              确认
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
};
