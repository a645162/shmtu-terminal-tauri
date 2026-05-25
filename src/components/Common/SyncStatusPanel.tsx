import React from 'react';
import {
  Badge,
  Button,
  Card,
  Spinner,
  Text,
} from '@fluentui/react-components';
import {
  CheckmarkCircle24Regular,
  Dismiss24Regular,
  ShieldTask24Regular,
  Warning24Regular,
} from '@fluentui/react-icons';
import { useAppStore } from '../../stores/appStore';

function getStatusMeta(status: string) {
  switch (status) {
    case 'running':
      return {
        label: '同步中',
        color: 'brand' as const,
        icon: <Spinner size="tiny" />,
      };
    case 'completed':
      return {
        label: '已完成',
        color: 'success' as const,
        icon: <CheckmarkCircle24Regular />,
      };
    case 'captcha_required':
      return {
        label: '等待验证码',
        color: 'warning' as const,
        icon: <ShieldTask24Regular />,
      };
    case 'error':
      return {
        label: '失败',
        color: 'danger' as const,
        icon: <Warning24Regular />,
      };
    default:
      return {
        label: '状态',
        color: 'informative' as const,
        icon: null,
      };
  }
}

export const SyncStatusPanel: React.FC = () => {
  const syncProgress = useAppStore((s) => s.syncProgress);
  const clearSyncProgress = useAppStore((s) => s.clearSyncProgress);

  if (!syncProgress || syncProgress.status === 'idle') {
    return null;
  }

  const meta = getStatusMeta(syncProgress.status);
  const detail =
    syncProgress.message ||
    (syncProgress.status === 'running'
      ? '正在处理同步任务...'
      : syncProgress.status === 'completed'
        ? `同步完成，本次新增 ${syncProgress.new_items} 条记录`
        : syncProgress.status === 'captcha_required'
          ? syncProgress.error || '需要输入验证码以继续'
          : syncProgress.error || '同步失败');
  const accountProgress =
    syncProgress.total_accounts && syncProgress.total_accounts > 0
      ? `${(syncProgress.account_index ?? 0) + 1}/${syncProgress.total_accounts}`
      : null;
  const currentAccount = syncProgress.current_account || syncProgress.account_id || null;

  return (
    <div
      style={{
        position: 'fixed',
        right: 20,
        bottom: 20,
        zIndex: 1000,
        width: 340,
        maxWidth: 'calc(100vw - 40px)',
      }}
    >
      <Card className="motion-hover-lift" style={{ padding: 14 }}>
        <div style={{ display: 'flex', alignItems: 'flex-start', gap: 12 }}>
          <div style={{ marginTop: 2 }}>{meta.icon}</div>
          <div style={{ flex: 1, minWidth: 0 }}>
            <div
              style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
                gap: 8,
                marginBottom: 6,
              }}
            >
              <Badge appearance="filled" color={meta.color}>
                {meta.label}
              </Badge>
              {syncProgress.status !== 'running' && (
                <Button
                  appearance="subtle"
                  icon={<Dismiss24Regular />}
                  size="small"
                  onClick={clearSyncProgress}
                />
              )}
            </div>
            {(accountProgress || currentAccount) && (
              <Text
                size={200}
                block
                style={{ color: 'var(--colorNeutralForeground3)', marginBottom: 6 }}
              >
                {accountProgress ? `账号进度 ${accountProgress}` : ''}
                {accountProgress && currentAccount ? ' · ' : ''}
                {currentAccount ? `当前账号 ${currentAccount}` : ''}
              </Text>
            )}
            <Text block style={{ whiteSpace: 'pre-wrap', wordBreak: 'break-word' }}>
              {detail}
            </Text>
          </div>
        </div>
      </Card>
    </div>
  );
};
