import React from 'react';
import {
  Dialog,
  DialogSurface,
  DialogTitle,
  DialogBody,
  DialogContent,
  DialogActions,
  Button,
  Text,
} from '@fluentui/react-components';
import { ErrorCircle24Regular } from '@fluentui/react-icons';
import { useAppStore } from '../../stores/appStore';

export const ErrorDialog: React.FC = () => {
  const showErrorDialog = useAppStore((s) => s.showErrorDialog);
  const errorMessage = useAppStore((s) => s.errorMessage);
  const setShowErrorDialog = useAppStore((s) => s.setShowErrorDialog);

  return (
    <Dialog open={showErrorDialog}>
      <DialogSurface style={{ maxWidth: 500 }}>
        <DialogBody>
          <DialogTitle>
            <ErrorCircle24Regular style={{ marginRight: 8, color: 'var(--colorPaletteRedForeground3)' }} />
            操作失败
          </DialogTitle>
          <DialogContent>
            <div
              style={{
                padding: 16,
                borderRadius: 8,
                backgroundColor: 'var(--colorNeutralBackground4)',
                marginBottom: 16,
              }}
            >
              <Text block style={{ whiteSpace: 'pre-wrap', wordBreak: 'break-word' }}>
                {errorMessage}
              </Text>
            </div>
            <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
              此错误已记录到日志文件中。
            </Text>
          </DialogContent>
          <DialogActions>
            <Button appearance="primary" onClick={() => setShowErrorDialog(false)}>
              确定
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
};