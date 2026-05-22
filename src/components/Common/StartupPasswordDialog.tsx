import React, { useState } from 'react';
import {
  Dialog,
  DialogTrigger,
  DialogSurface,
  DialogTitle,
  DialogBody,
  DialogContent,
  DialogActions,
  Input,
  Button,
  Text,
  MessageBar,
  MessageBarBody,
} from '@fluentui/react-components';
import { Key24Regular } from '@fluentui/react-icons';
import { useAppStore } from '../../stores/appStore';
import * as tauri from '../../services/tauri';

export const StartupPasswordDialog: React.FC = () => {
  const setShowStartupDialog = useAppStore((s) => s.setShowStartupDialog);
  const [password, setPassword] = useState('');
  const [error, setError] = useState('');
  const [isLoading, setIsLoading] = useState(false);

  const handleVerify = async () => {
    if (!password.trim()) {
      setError('请输入密码');
      return;
    }
    setIsLoading(true);
    setError('');
    try {
      const valid = await tauri.verify_startup_password(password);
      if (valid) {
        setShowStartupDialog(false);
      } else {
        setError('密码错误，请重试');
      }
    } catch {
      setError('验证失败');
    } finally {
      setIsLoading(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      handleVerify();
    }
  };

  return (
    <Dialog open>
      <DialogSurface>
        <DialogBody>
          <DialogTitle>
            <Key24Regular style={{ marginRight: 8 }} />
            请输入密码
          </DialogTitle>
          <DialogContent>
            <Text block style={{ marginBottom: 16 }}>
              应用已设置密码保护，请输入密码以继续。
            </Text>
            {error && (
              <MessageBar intent="error" style={{ marginBottom: 12 }}>
                <MessageBarBody>{error}</MessageBarBody>
              </MessageBar>
            )}
            <Input
              type="password"
              placeholder="请输入密码"
              value={password}
              onChange={(e) => setPassword(e.currentTarget.value)}
              onKeyDown={handleKeyDown}
              style={{ width: '100%' }}
              autoFocus
            />
          </DialogContent>
          <DialogActions>
            <Button appearance="primary" onClick={handleVerify} disabled={isLoading}>
              确认
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
};
