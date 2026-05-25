import React, { useState, useEffect } from 'react';
import {
  Dialog,
  DialogSurface,
  DialogTitle,
  DialogBody,
  DialogContent,
  DialogActions,
  Button,
  Text,
  Divider,
  Link,
} from '@fluentui/react-components';
import { Info24Regular } from '@fluentui/react-icons';
import { useAppStore } from '../../stores/appStore';
import * as tauri from '../../services/tauri';
import {
  SectionEnterMotion,
} from '../../components/Common/motion';

export const AboutDialog: React.FC = () => {
  const showAboutDialog = useAppStore((s) => s.showAboutDialog);
  const setShowAboutDialog = useAppStore((s) => s.setShowAboutDialog);

  const [version, setVersion] = useState('0.1.0');
  const [checkingUpdate, setCheckingUpdate] = useState(false);
  const [updateInfo, setUpdateInfo] = useState<string | null>(null);

  useEffect(() => {
    tauri.get_app_version().then(setVersion).catch(() => {});
  }, []);

  const handleCheckUpdate = async () => {
    setCheckingUpdate(true);
    setUpdateInfo(null);
    try {
      const latest = await tauri.check_for_updates();
      if (latest) {
        setUpdateInfo(`发现新版本: ${latest}`);
      } else {
        setUpdateInfo('当前已是最新版本');
      }
    } catch {
      setUpdateInfo('检查更新失败');
    } finally {
      setCheckingUpdate(false);
    }
  };

  return (
    <Dialog open={showAboutDialog} onOpenChange={(_, data) => !data.open && setShowAboutDialog(false)}>
      <DialogSurface style={{ maxWidth: 420 }}>
        <DialogBody>
          <DialogTitle>
            <Info24Regular style={{ marginRight: 8 }} />
            关于 海大终端
          </DialogTitle>
          <DialogContent>
            <SectionEnterMotion>
              <div className="motion-sheen" style={{ textAlign: 'center', padding: '16px 0' }}>
                <Text size={700} weight="bold" block style={{ marginBottom: 4 }}>
                  海大终端
                </Text>
                <Text size={300} style={{ color: 'var(--colorNeutralForeground3)' }}>
                  版本 {version}
                </Text>
              </div>
            </SectionEnterMotion>

            <Divider style={{ margin: '12px 0' }} />

            <Text block style={{ marginBottom: 8 }}>
              上海海事大学校园终端应用
            </Text>

            <div style={{ display: 'grid', gap: 4 }}>
              <Text size={200}>
                <strong>开发者:</strong> Haomin Kong
              </Text>
              <Text size={200}>
                <strong>GitHub:</strong>{' '}
                <Link href="https://github.com/a645162" target="_blank">
                  github.com/a645162
                </Link>
              </Text>
            </div>

            <Divider style={{ margin: '12px 0' }} />

            <Text size={200} weight="semibold" block style={{ marginBottom: 4 }}>
              技术栈 (Tauri端)
            </Text>
            <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
              Tauri v2 + React 19 + TypeScript
            </Text>
            <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
              Fluent UI + Recharts + Zustand
            </Text>

            <Divider style={{ margin: '12px 0' }} />

            {updateInfo && (
              <Text
                size={200}
                block
                style={{
                  marginBottom: 8,
                  color: updateInfo.includes('失败') ? 'var(--colorPaletteRedForeground3)' : 'var(--colorPaletteGreenForeground3)',
                }}
              >
                {updateInfo}
              </Text>
            )}
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" onClick={handleCheckUpdate} disabled={checkingUpdate}>
              {checkingUpdate ? '检查中...' : '检查更新'}
            </Button>
            <Button appearance="secondary" onClick={() => setShowAboutDialog(false)}>
              关闭
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
};
