import React, { useState, useEffect, useCallback } from 'react';
import {
  Dialog, DialogSurface, DialogTitle, DialogBody, DialogContent, DialogActions,
  Button, Input, Text, Label, Spinner, MessageBar, MessageBarBody,
  TabList, Tab, Table, TableHeader, TableRow, TableCell, TableBody, TableHeaderCell,
} from '@fluentui/react-components';
import { useAppStore } from '../../stores/appStore';
import * as tauri from '../../services/tauri';

type RemoteTab = 'info' | 'connect' | 'devices';

export const RemoteAccessDialog: React.FC = () => {
  const showRemoteDialog = useAppStore((s) => s.showRemoteDialog);
  const setShowRemoteDialog = useAppStore((s) => s.setShowRemoteDialog);
  const remoteSessions = useAppStore((s) => s.remoteSessions);
  const setRemoteSessions = useAppStore((s) => s.setRemoteSessions);
  const setMessage = useAppStore((s) => s.setMessage);
  const showError = useAppStore((s) => s.showError);

  const [selectedTab, setSelectedTab] = useState<RemoteTab>('info');
  const [connectUrl, setConnectUrl] = useState('');
  const [connecting, setConnecting] = useState(false);
  const [exporting, setExporting] = useState<string | null>(null);

  const loadSessions = useCallback(async () => {
    try {
      const list = await tauri.remote_list_sessions();
      setRemoteSessions(list);
    } catch (e) {
      console.error('Failed to load remote sessions:', e);
    }
  }, [setRemoteSessions]);

  useEffect(() => {
    if (showRemoteDialog) {
      loadSessions();
    }
  }, [showRemoteDialog, loadSessions]);

  const renderInfoTab = () => (
    <div style={{ display: 'grid', gap: 16 }}>
      <Text weight="semibold" size={400}>本机远程访问信息</Text>
      <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
        启动 Web 服务器后，对端设备通过浏览器或客户端连接此 URL + Token 即可访问本机账单数据。
      </Text>
      <Text>本端 Tauri 应用使用 RESTful HTTP 调用对方 Web 服务器。</Text>
      <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
        请在设置中启动 Web 服务器。
      </Text>
    </div>
  );

  const renderConnectTab = () => (
    <div style={{ display: 'grid', gap: 16 }}>
      <Text weight="semibold" size={400}>连接到远程设备</Text>
      <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
        输入对方设备 Web 服务器的完整 URL（如 http://192.168.1.100:8080/）
      </Text>
      <div>
        <Label>对方设备 URL</Label>
        <Input
          value={connectUrl}
          onChange={(_, d) => setConnectUrl(d.value)}
          placeholder="http://192.168.1.100:8080/"
          style={{ width: '100%' }}
        />
      </div>
      <Button
        appearance="primary"
        onClick={async () => {
          if (!connectUrl.trim()) return;
          setConnecting(true);
          try {
            const session = await tauri.remote_connect(connectUrl.trim(), 'Tauri Desktop');
            setMessage(`已连接到 ${session.device_name}`);
            setConnectUrl('');
            await loadSessions();
            setSelectedTab('devices');
          } catch (e) {
            showError(`连接失败: ${e}`);
          } finally {
            setConnecting(false);
          }
        }}
        disabled={connecting || !connectUrl.trim()}
      >
        {connecting ? <Spinner size="tiny" /> : '连接'}
      </Button>
    </div>
  );

  const renderDevicesTab = () => (
    <div style={{ display: 'grid', gap: 16 }}>
      <Text weight="semibold" size={400}>已配对设备</Text>
      {remoteSessions.length === 0 ? (
        <Text style={{ color: 'var(--colorNeutralForeground3)' }}>暂无已配对设备</Text>
      ) : (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHeaderCell>设备名</TableHeaderCell>
              <TableHeaderCell>URL</TableHeaderCell>
              <TableHeaderCell>操作</TableHeaderCell>
            </TableRow>
          </TableHeader>
          <TableBody>
            {remoteSessions.map((session) => (
              <TableRow key={session.session_id}>
                <TableCell>{session.device_name}</TableCell>
                <TableCell><code style={{ fontSize: 12 }}>{session.base_url}</code></TableCell>
                <TableCell>
                  <div style={{ display: 'flex', gap: 8 }}>
                    <Button
                      size="small"
                      appearance="primary"
                      disabled={exporting !== null}
                      onClick={async () => {
                        setExporting(session.session_id);
                        try {
                          const data = await tauri.remote_export(session.session_id);
                          setMessage(`已导出 ${data.length} 字节数据`);
                        } catch (e) {
                          showError(`导出失败: ${e}`);
                        } finally {
                          setExporting(null);
                        }
                      }}
                    >
                      {exporting === session.session_id ? <Spinner size="tiny" /> : '导出数据'}
                    </Button>
                    <Button
                      size="small"
                      appearance="subtle"
                      onClick={async () => {
                        try {
                          await tauri.remote_disconnect(session.session_id);
                          await loadSessions();
                        } catch (e) {
                          showError(`断开失败: ${e}`);
                        }
                      }}
                    >
                      断开
                    </Button>
                  </div>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}
    </div>
  );

  return (
    <Dialog
      open={showRemoteDialog}
      onOpenChange={(_, data) => !data.open && setShowRemoteDialog(false)}
    >
      <DialogSurface style={{ maxWidth: 600 }}>
        <DialogBody>
          <DialogTitle>远程访问</DialogTitle>
          <DialogContent>
            <TabList
              selectedValue={selectedTab}
              onTabSelect={(_, d) => setSelectedTab(d.value as RemoteTab)}
              style={{ marginBottom: 16 }}
            >
              <Tab value="info">本机信息</Tab>
              <Tab value="connect">连接对端</Tab>
              <Tab value="devices">已配对</Tab>
            </TabList>
            {selectedTab === 'info' && renderInfoTab()}
            {selectedTab === 'connect' && renderConnectTab()}
            {selectedTab === 'devices' && renderDevicesTab()}
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" onClick={() => setShowRemoteDialog(false)}>
              关闭
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
};
