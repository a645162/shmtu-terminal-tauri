import React, { useState, useEffect, useCallback, useRef } from 'react';
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
  Label,
  Spinner,
  MessageBar,
  MessageBarBody,
  TabList,
  Tab,
  Table,
  TableHeader,
  TableRow,
  TableCell,
  TableBody,
  TableHeaderCell,
  ProgressBar,
  Select,
} from '@fluentui/react-components';
import {
  People24Regular,
  QrCode24Regular,
  Link24Regular,
  Clipboard24Regular,
} from '@fluentui/react-icons';
import { listen } from '@tauri-apps/api/event';
import QRCode from 'qrcode';
import { useAppStore } from '../../stores/appStore';
import * as tauri from '../../services/tauri';
import type {
  P2PTransferProgress,
  P2PPairingRequest,
  P2PTransferComplete,
  P2PTransferError,
  P2PEncryptionStatus,
  P2PDataReceived,
} from '../../services/tauri';
import {
  PageEnterMotion,
  SectionEnterMotion,
} from '../../components/Common/motion';

type P2PTab = 'qr' | 'connect' | 'devices' | 'history';

interface TransferRecord {
  sessionId: string;
  billCount: number;
  direction: 'send' | 'receive';
  timestamp: number;
  success: boolean;
  errorMessage?: string;
}

const PAIR_CODE_LENGTH = 6;

export const P2PTransferDialog: React.FC = () => {
  const showP2PDialog = useAppStore((s) => s.showP2PDialog);
  const setShowP2PDialog = useAppStore((s) => s.setShowP2PDialog);
  const p2pStatus = useAppStore((s) => s.p2pStatus);
  const loadP2PStatus = useAppStore((s) => s.loadP2PStatus);
  const p2pTransferProgress = useAppStore((s) => s.p2pTransferProgress);
  const setP2PTransferProgress = useAppStore((s) => s.setP2PTransferProgress);
  const pendingPairRequest = useAppStore((s) => s.pendingPairRequest);
  const setPendingPairRequest = useAppStore((s) => s.setPendingPairRequest);
  const identities = useAppStore((s) => s.identities);
  const currentIdentity = useAppStore((s) => s.currentIdentity);
  const config = useAppStore((s) => s.config);

  const showError = useAppStore((s) => s.showError);

  const [selectedTab, setSelectedTab] = useState<P2PTab>('qr');
  const [qrDataUrl, setQrDataUrl] = useState<string | null>(null);
  const [qrLoading, setQrLoading] = useState(false);
  const [serverStarting, setServerStarting] = useState(false);
  const [serverStopping, setServerStopping] = useState(false);
  const [message, setMessage] = useState('');

  // Connect tab state
  const [connectIp, setConnectIp] = useState('');
  const [connectPort, setConnectPort] = useState('19827');
  const [connectPairCode, setConnectPairCode] = useState('');
  const [connecting, setConnecting] = useState(false);

  // Send bills state
  const [sendingSessionId, setSendingSessionId] = useState<string | null>(null);
  const [sendIdentityId, setSendIdentityId] = useState<number | undefined>(undefined);
  const [sendSyncRange, setSendSyncRange] = useState<tauri.SyncRangePreset>('all');

  // Transfer history
  const [transferHistory, setTransferHistory] = useState<TransferRecord[]>([]);

  // Pairing prompt state
  const [pairPrompt, setPairPrompt] = useState<P2PPairingRequest | null>(null);
  const [pairProcessing, setPairProcessing] = useState(false);

  // Encryption state
  const [encryptionMap, setEncryptionMap] = useState<Record<string, 'negotiating' | 'established' | 'failed'>>({});
  const [encryptionMethods, setEncryptionMethods] = useState<Record<string, string>>({});

  // Data received state
  const [receivedData, setReceivedData] = useState<P2PDataReceived | null>(null);
  const [importIdentityId, setImportIdentityId] = useState(currentIdentity?.id?.toString() ?? '');
  const [importing, setImporting] = useState(false);

  // Encryption warning state
  const [encryptionWarning, setEncryptionWarning] = useState<string | null>(null);

  const unlistenRefs = useRef<Array<() => void>>([]);

  // Generate QR code from payload
  const generateQR = useCallback(async () => {
    if (!p2pStatus?.server_running) {
      setQrDataUrl(null);
      return;
    }
    setQrLoading(true);
    try {
      const payload = await tauri.p2p_get_qr_payload();
      const dataUrl = await QRCode.toDataURL(payload, {
        width: 320,
        margin: 2,
        color: {
          dark: '#000000',
          light: '#FFFFFF',
        },
      });
      setQrDataUrl(dataUrl);
    } catch (e) {
      console.error('Failed to generate QR code:', e);
      setQrDataUrl(null);
    } finally {
      setQrLoading(false);
    }
  }, [p2pStatus?.server_running]);

  // Load P2P status on open
  useEffect(() => {
    if (showP2PDialog) {
      loadP2PStatus();
    }
  }, [showP2PDialog, loadP2PStatus]);

  // Generate QR when server is running
  useEffect(() => {
    if (p2pStatus?.server_running && selectedTab === 'qr') {
      generateQR();
    } else {
      setQrDataUrl(null);
    }
  }, [p2pStatus?.server_running, selectedTab, generateQR]);

  // Periodically refresh P2P status
  useEffect(() => {
    if (!showP2PDialog) return;
    const interval = window.setInterval(() => {
      loadP2PStatus();
    }, 3000);
    return () => window.clearInterval(interval);
  }, [showP2PDialog, loadP2PStatus]);

  // Listen for Tauri P2P events
  useEffect(() => {
    if (!showP2PDialog) return;

    const setupListeners = async () => {
      const unlistenPairing = await listen<P2PPairingRequest>('p2p-pairing-request', (event) => {
        setPairPrompt(event.payload);
        setPendingPairRequest(event.payload);
      });

      const unlistenProgress = await listen<P2PTransferProgress>(
        'p2p-transfer-progress',
        (event) => {
          setP2PTransferProgress(event.payload);
        }
      );

      const unlistenComplete = await listen<P2PTransferComplete>('p2p-transfer-complete', (event) => {
        setP2PTransferProgress(null);
        setTransferHistory((prev) => [
          {
            sessionId: event.payload.session_id,
            billCount: event.payload.bill_count,
            direction: event.payload.direction,
            timestamp: Date.now(),
            success: true,
          },
          ...prev,
        ]);
        setMessage(
          `${event.payload.direction === 'send' ? '发送' : '接收'}完成: ${event.payload.bill_count} 条账单`
        );
      });

      const unlistenError = await listen<P2PTransferError>('p2p-transfer-error', (event) => {
        setP2PTransferProgress(null);
        setTransferHistory((prev) => [
          {
            sessionId: event.payload.session_id,
            billCount: 0,
            direction: event.payload.direction,
            timestamp: Date.now(),
            success: false,
            errorMessage: event.payload.error,
          },
          ...prev,
        ]);
        showError(`传输失败: ${event.payload.error}`);
      });

      const unlistenEncryptionNegotiating = await listen<P2PEncryptionStatus>(
        'p2p-encryption-negotiating',
        (event) => {
          setEncryptionMap((prev) => ({
            ...prev,
            [event.payload.session_id]: 'negotiating',
          }));
          setMessage('正在建立加密连接...');
        }
      );

      const unlistenEncryptionEstablished = await listen<P2PEncryptionStatus>(
        'p2p-encryption-established',
        (event) => {
          setEncryptionMap((prev) => ({
            ...prev,
            [event.payload.session_id]: 'established',
          }));
          if (event.payload.method) {
            setEncryptionMethods((prev) => ({
              ...prev,
              [event.payload.session_id]: event.payload.method!,
            }));
          }
          setEncryptionWarning(null);
          setMessage(`加密连接已建立 (${event.payload.method || 'AES-256-GCM'})`);
        }
      );

      const unlistenEncryptionFailed = await listen<P2PEncryptionStatus>(
        'p2p-encryption-failed',
        (event) => {
          setEncryptionMap((prev) => ({
            ...prev,
            [event.payload.session_id]: 'failed',
          }));
          setEncryptionWarning('连接未加密，数据传输可能不安全');
        }
      );

      const unlistenDataReceived = await listen<P2PDataReceived>(
        'p2p-data-received',
        (event) => {
          setReceivedData(event.payload);
        }
      );

      unlistenRefs.current = [
        unlistenPairing,
        unlistenProgress,
        unlistenComplete,
        unlistenError,
        unlistenEncryptionNegotiating,
        unlistenEncryptionEstablished,
        unlistenEncryptionFailed,
        unlistenDataReceived,
      ];
    };

    setupListeners().catch(console.error);

    return () => {
      unlistenRefs.current.forEach((fn) => fn());
      unlistenRefs.current = [];
    };
  }, [showP2PDialog, setP2PTransferProgress, setPendingPairRequest, showError]);

  // Handle store-level pending pair request (from external listeners)
  useEffect(() => {
    if (pendingPairRequest && !pairPrompt) {
      setPairPrompt(pendingPairRequest);
    }
  }, [pendingPairRequest, pairPrompt]);

  useEffect(() => {
    if (!pairPrompt || pairProcessing) {
      return;
    }

    const isTrustedReconnect = p2pStatus?.sessions.some((session) => {
      if (!session.is_paired) {
        return false;
      }
      if (session.peer_device_name !== pairPrompt.peer_device_name) {
        return false;
      }
      return session.peer_ip === pairPrompt.peer_ip;
    });

    if (isTrustedReconnect) {
      void handleAcceptPairing();
    }
  }, [pairPrompt, pairProcessing, p2pStatus]);

  useEffect(() => {
    if (!pairPrompt || !config?.p2p?.auto_accept || pairProcessing) {
      return;
    }
    void handleAcceptPairing();
  }, [pairPrompt, config?.p2p?.auto_accept, pairProcessing]);

  const handleStartServer = async () => {
    setServerStarting(true);
    setMessage('');
    try {
      await tauri.p2p_start_server();
      await loadP2PStatus();
      setMessage('P2P 服务已启动');
    } catch (e) {
      showError(`启动点对点服务失败: ${e}`);
    } finally {
      setServerStarting(false);
    }
  };

  const handleStopServer = async () => {
    setServerStopping(true);
    setMessage('');
    try {
      await tauri.p2p_stop_server();
      await loadP2PStatus();
      setQrDataUrl(null);
      setMessage('P2P 服务已停止');
    } catch (e) {
      showError(`停止点对点服务失败: ${e}`);
    } finally {
      setServerStopping(false);
    }
  };

  const handleConnect = async () => {
    if (!connectIp.trim() || !connectPort.trim() || connectPairCode.trim().length !== PAIR_CODE_LENGTH) return;
    setConnecting(true);
    setMessage('');
    try {
      await tauri.p2p_connect(
        connectIp.trim(),
        parseInt(connectPort.trim()),
        connectPairCode.trim(),
        config?.p2p?.device_name || 'shmtu-terminal'
      );
      await loadP2PStatus();
      setMessage('连接成功');
      setConnectIp('');
      setConnectPairCode('');
      setSelectedTab('devices');
    } catch (e) {
      showError(`连接失败: ${e}`);
    } finally {
      setConnecting(false);
    }
  };

  const handleAcceptPairing = async () => {
    if (!pairPrompt) return;
    setPairProcessing(true);
    try {
      await tauri.p2p_accept_pairing(pairPrompt.session_id);
      setPairPrompt(null);
      setPendingPairRequest(null);
      await loadP2PStatus();
      setMessage(`已接受 ${pairPrompt.peer_device_name} 的配对请求`);
    } catch (e) {
      showError(`接受配对失败: ${e}`);
    } finally {
      setPairProcessing(false);
    }
  };

  const handleRejectPairing = async () => {
    if (!pairPrompt) return;
    setPairProcessing(true);
    try {
      await tauri.p2p_reject_pairing(pairPrompt.session_id);
      setPairPrompt(null);
      setPendingPairRequest(null);
      setMessage('已拒绝配对请求');
    } catch (e) {
      showError(`拒绝配对失败: ${e}`);
    } finally {
      setPairProcessing(false);
    }
  };

  const handleSendBills = async (sessionId: string) => {
    setSendingSessionId(sessionId);
    setMessage('');
    try {
      await tauri.p2p_send_bills(sessionId, sendIdentityId, sendSyncRange);
      setMessage('账单发送中...');
    } catch (e) {
      showError(`发送账单失败: ${e}`);
    } finally {
      setSendingSessionId(null);
    }
  };

  const handleDisconnect = async (sessionId: string) => {
    const session = p2pStatus?.sessions.find((item) => item.session_id === sessionId);
    try {
      await tauri.p2p_disconnect(sessionId);
      await loadP2PStatus();
      setMessage(session?.is_connected ? '已断开连接' : '已移除已配对设备');
    } catch (e) {
      showError(`断开连接失败: ${e}`);
    }
  };

  const handleReconnect = async (sessionId: string) => {
    try {
      await tauri.p2p_reconnect(sessionId);
      await loadP2PStatus();
      setMessage('重连成功');
    } catch (e) {
      showError(`重连失败: ${e}`);
    }
  };

  const handleImportReceivedData = async () => {
    if (!receivedData || !importIdentityId) return;
    setImporting(true);
    try {
      const identityId = parseInt(importIdentityId);
      const count = await tauri.p2p_import_received_data(
        receivedData.session_id,
        identityId
      );
      setMessage(`导入成功: ${count} 条账单已导入`);
      setReceivedData(null);
    } catch (e) {
      showError(`导入失败: ${e}`);
    } finally {
      setImporting(false);
    }
  };

  const renderEncryptionBadge = (sessionId: string) => {
    const status = encryptionMap[sessionId];
    if (status === 'established') {
      const method = encryptionMethods[sessionId];
      return method ? `已加密 (${method})` : '已加密';
    }
    if (status === 'negotiating') {
      return '协商中...';
    }
    if (status === 'failed') {
      return '未加密 (协商失败)';
    }
    return '未加密';
  };

  const renderQRTab = () => (
    <div style={{ display: 'grid', gap: 16, alignItems: 'center' }}>
      <Text weight="semibold" size={400}>
        本机二维码
      </Text>

      {!p2pStatus?.server_running ? (
        <div style={{ textAlign: 'center', padding: 24 }}>
          <Text block style={{ color: 'var(--colorNeutralForeground3)', marginBottom: 16 }}>
            点对点服务未启动，请先启动服务以生成二维码
          </Text>
          <Button
            appearance="primary"
            onClick={handleStartServer}
            disabled={serverStarting}
            size="large"
          >
            {serverStarting ? <Spinner size="tiny" /> : '启动点对点服务'}
          </Button>
        </div>
      ) : (
        <>
          <div style={{ textAlign: 'center' }}>
            {qrLoading ? (
              <Spinner label="生成二维码中..." />
            ) : qrDataUrl ? (
              <div
                style={{
                  display: 'inline-block',
                  padding: 16,
                  background: '#FFFFFF',
                  borderRadius: 12,
                  boxShadow: '0 2px 8px rgba(0,0,0,0.1)',
                }}
              >
                <img
                  src={qrDataUrl}
                  alt="点对点 QR Code"
                  style={{ width: 280, height: 280, display: 'block' }}
                />
              </div>
            ) : (
              <Text style={{ color: 'var(--colorNeutralForeground3)' }}>
                二维码生成失败
              </Text>
            )}
          </div>

          {p2pStatus?.pair_code && (
            <div
              style={{
                textAlign: 'center',
                padding: 12,
                borderRadius: 10,
                border: '1px solid var(--colorNeutralStroke2)',
                background: 'var(--colorNeutralBackground2)',
              }}
            >
              <Text
                size={600}
                weight="bold"
                block
                style={{ marginBottom: 8, fontVariantNumeric: 'tabular-nums', letterSpacing: '0.15em' }}
              >
                配对码: {p2pStatus.pair_code.replace(/(.{3})/g, '$1 ').trim()}
              </Text>
              <Text size={200} block style={{ color: 'var(--colorNeutralForeground3)' }}>
                端口: {p2pStatus.port}
              </Text>
              {p2pStatus.local_ips.length > 0 && (
                <Text size={200} block style={{ color: 'var(--colorNeutralForeground3)' }}>
                  IP: {p2pStatus.local_ips.join(', ')}
                </Text>
              )}
              {p2pStatus?.encryption_method && (
                <Text size={200} block style={{ color: 'var(--colorNeutralForeground3)' }}>
                  传输加密: {p2pStatus.encryption_method}
                </Text>
              )}
            </div>
          )}

          <div style={{ textAlign: 'center' }}>
            <Button
              appearance="subtle"
              onClick={handleStopServer}
              disabled={serverStopping}
            >
              {serverStopping ? <Spinner size="tiny" /> : '停止点对点服务'}
            </Button>
          </div>
        </>
      )}
    </div>
  );

  const renderConnectTab = () => (
    <div style={{ display: 'grid', gap: 16 }}>
      <Text weight="semibold" size={400}>
        扫描配对
      </Text>
      <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
        输入目标设备的 IP 地址、端口和{PAIR_CODE_LENGTH}位配对码进行连接
      </Text>
      <div>
        <Label>IP 地址</Label>
        <Input
          value={connectIp}
          onChange={(e) => setConnectIp(e.currentTarget.value)}
          placeholder="如: 192.168.1.100"
          style={{ width: '100%' }}
        />
      </div>
      <div>
        <Label>端口</Label>
        <Input
          value={connectPort}
          onChange={(e) => setConnectPort(e.currentTarget.value)}
          placeholder="19827"
          style={{ width: '100%' }}
        />
      </div>
      <div>
        <Label>配对码</Label>
        <Input
          value={connectPairCode}
          onChange={(e) => setConnectPairCode(e.currentTarget.value.toUpperCase())}
          placeholder={`输入对方显示的${PAIR_CODE_LENGTH}位配对码`}
          maxLength={PAIR_CODE_LENGTH}
          style={{ width: '100%' }}
        />
      </div>
      <Button
        appearance="primary"
        onClick={handleConnect}
        disabled={
          connecting ||
          !connectIp.trim() ||
          !connectPort.trim() ||
          connectPairCode.trim().length !== PAIR_CODE_LENGTH
        }
      >
        {connecting ? <Spinner size="tiny" /> : '连接'}
      </Button>
    </div>
  );

  const renderDevicesTab = () => (
    <div style={{ display: 'grid', gap: 16 }}>
      <Text weight="semibold" size={400}>
        已配对设备
      </Text>
      {!p2pStatus?.server_running ? (
        <div style={{ textAlign: 'center', padding: 24 }}>
          <Text style={{ color: 'var(--colorNeutralForeground3)' }}>
            点对点服务未启动，请先在"本机二维码"标签页启动服务
          </Text>
        </div>
      ) : p2pStatus.sessions.length === 0 ? (
        <Text style={{ color: 'var(--colorNeutralForeground3)' }}>
          暂无已配对设备
        </Text>
      ) : (
        <>
          {/* Send options */}
          <div
            style={{
              padding: 12,
              borderRadius: 10,
              border: '1px solid var(--colorNeutralStroke2)',
              background: 'var(--colorNeutralBackground2)',
              display: 'grid',
              gap: 12,
            }}
          >
            <Text size={200} weight="semibold">发送选项</Text>
            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
              <div>
                <Label size="small" style={{ marginBottom: 4, display: 'block' }}>身份</Label>
                <Select
                  value={sendIdentityId?.toString() ?? ''}
                  onChange={(e) => {
                    const val = e.currentTarget.value;
                    setSendIdentityId(val ? parseInt(val) : undefined);
                  }}
                  size="small"
                  style={{ width: '100%' }}
                >
                  <option value="">全部身份</option>
                  {identities.map((id) => (
                    <option key={id.id} value={id.id.toString()}>{id.name}</option>
                  ))}
                </Select>
              </div>
              <div>
                <Label size="small" style={{ marginBottom: 4, display: 'block' }}>时间范围</Label>
                <Select
                  value={sendSyncRange}
                  onChange={(e) => setSendSyncRange(e.currentTarget.value as tauri.SyncRangePreset)}
                  size="small"
                  style={{ width: '100%' }}
                >
                  <option value="week">最近一周</option>
                  <option value="half_month">最近半月</option>
                  <option value="month">最近一月</option>
                  <option value="half_year">最近半年</option>
                  <option value="all">全部</option>
                </Select>
              </div>
            </div>
          </div>

          <Table>
            <TableHeader>
              <TableRow>
                <TableHeaderCell>设备名</TableHeaderCell>
                <TableHeaderCell>地址</TableHeaderCell>
                <TableHeaderCell>方向</TableHeaderCell>
                <TableHeaderCell>状态</TableHeaderCell>
                <TableHeaderCell>加密</TableHeaderCell>
                <TableHeaderCell>操作</TableHeaderCell>
              </TableRow>
            </TableHeader>
            <TableBody>
              {p2pStatus.sessions.map((session) => (
                <TableRow key={session.session_id}>
                  <TableCell>{session.peer_device_name}</TableCell>
                  <TableCell>{session.peer_ip}:{session.peer_port}</TableCell>
                  <TableCell>
                    {session.is_incoming ? '接收' : '发送'}
                  </TableCell>
                  <TableCell>
                    {session.is_connected ? '已连接' : '未连接'}
                  </TableCell>
                  <TableCell>
                    {renderEncryptionBadge(session.session_id)}
                  </TableCell>
                  <TableCell>
                    <div style={{ display: 'flex', gap: 8 }}>
                      <Button
                        size="small"
                        appearance="primary"
                        disabled={
                          !session.is_paired || !session.is_connected || sendingSessionId === session.session_id
                        }
                        onClick={() => handleSendBills(session.session_id)}
                      >
                        {sendingSessionId === session.session_id ? (
                          <Spinner size="tiny" />
                        ) : (
                          '发送账单'
                        )}
                      </Button>
                      <Button
                        size="small"
                        appearance="secondary"
                        disabled={session.is_connected}
                        onClick={() => handleReconnect(session.session_id)}
                      >
                        重连
                      </Button>
                      <Button
                        size="small"
                        appearance="subtle"
                        onClick={() => handleDisconnect(session.session_id)}
                      >
                        {session.is_connected ? '断开' : '移除'}
                      </Button>
                    </div>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </>
      )}

      {p2pTransferProgress && (
        <div
          style={{
            padding: 12,
            borderRadius: 10,
            border: '1px solid var(--colorNeutralStroke2)',
            background: 'var(--colorNeutralBackground2)',
            display: 'grid',
            gap: 8,
          }}
        >
          <Text size={200}>
            传输中
          </Text>
          <ProgressBar
            value={p2pTransferProgress.percentage / 100}
          />
          <Text size={100} style={{ color: 'var(--colorNeutralForeground3)' }}>
            {p2pTransferProgress.bytes_transferred} / {p2pTransferProgress.total_size} 字节 ({p2pTransferProgress.percentage.toFixed(1)}%)
          </Text>
        </div>
      )}
    </div>
  );

  const renderHistoryTab = () => (
    <div style={{ display: 'grid', gap: 16 }}>
      <Text weight="semibold" size={400}>
        传输记录
      </Text>
      {transferHistory.length === 0 ? (
        <Text style={{ color: 'var(--colorNeutralForeground3)' }}>
          暂无传输记录
        </Text>
      ) : (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHeaderCell>时间</TableHeaderCell>
              <TableHeaderCell>方向</TableHeaderCell>
              <TableHeaderCell>账单数</TableHeaderCell>
              <TableHeaderCell>状态</TableHeaderCell>
            </TableRow>
          </TableHeader>
          <TableBody>
            {transferHistory.map((record, index) => (
              <TableRow key={`${record.sessionId}-${record.timestamp}-${index}`}>
                <TableCell>
                  {new Date(record.timestamp).toLocaleTimeString('zh-CN')}
                </TableCell>
                <TableCell>
                  {record.direction === 'send' ? '发送' : '接收'}
                </TableCell>
                <TableCell>{record.success ? `${record.billCount} 条` : '-'}</TableCell>
                <TableCell>
                  {record.success ? '成功' : `失败${record.errorMessage ? ': ' + record.errorMessage : ''}`}
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}
    </div>
  );

  const renderContent = () => {
    switch (selectedTab) {
      case 'qr':
        return renderQRTab();
      case 'connect':
        return renderConnectTab();
      case 'devices':
        return renderDevicesTab();
      case 'history':
        return renderHistoryTab();
    }
  };

  return (
    <>
      <Dialog
        open={showP2PDialog}
        onOpenChange={(_, data) => !data.open && setShowP2PDialog(false)}
      >
        <DialogSurface style={{ maxWidth: 600 }}>
          <DialogBody>
            <DialogTitle>点对点互传</DialogTitle>
            <DialogContent>
              <SectionEnterMotion>
                <div>
                  <TabList
                    selectedValue={selectedTab}
                    onTabSelect={(_, data) => setSelectedTab(data.value as P2PTab)}
                    style={{ marginBottom: 16 }}
                  >
                    <Tab icon={<QrCode24Regular />} value="qr">
                      本机二维码
                    </Tab>
                    <Tab icon={<Link24Regular />} value="connect">
                      扫描配对
                    </Tab>
                    <Tab icon={<People24Regular />} value="devices">
                      已配对设备
                    </Tab>
                    <Tab icon={<Clipboard24Regular />} value="history">
                      传输记录
                    </Tab>
                  </TabList>
                </div>
              </SectionEnterMotion>

              {encryptionWarning && (
                <MessageBar intent="warning" style={{ marginBottom: 12 }}>
                  <MessageBarBody>{encryptionWarning}</MessageBarBody>
                </MessageBar>
              )}

              {message && (
                <MessageBar
                  intent={message.includes('失败') ? 'error' : 'success'}
                  style={{ marginBottom: 12 }}
                >
                  <MessageBarBody>{message}</MessageBarBody>
                </MessageBar>
              )}

              <PageEnterMotion key={selectedTab}>
                <div>{renderContent()}</div>
              </PageEnterMotion>
            </DialogContent>
            <DialogActions>
              <Button
                appearance="secondary"
                onClick={() => setShowP2PDialog(false)}
              >
                关闭
              </Button>
            </DialogActions>
          </DialogBody>
        </DialogSurface>
      </Dialog>

      {/* Pairing Request Prompt Dialog */}
      {pairPrompt && (
        <Dialog open={true}>
          <DialogSurface style={{ maxWidth: 400 }}>
            <DialogBody>
              <DialogTitle>配对请求</DialogTitle>
              <DialogContent>
                <div style={{ display: 'grid', gap: 12, textAlign: 'center' }}>
                  <Text size={400}>
                    设备 <Text weight="bold">{pairPrompt.peer_device_name}</Text> 请求配对
                  </Text>
                  <Text size={300} style={{ color: 'var(--colorNeutralForeground3)' }}>
                    来自: {pairPrompt.peer_ip}:{pairPrompt.peer_port}
                  </Text>
                  <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
                    请确认是否接受配对请求
                  </Text>
                </div>
              </DialogContent>
              <DialogActions>
                <Button
                  appearance="secondary"
                  onClick={handleRejectPairing}
                  disabled={pairProcessing}
                >
                  拒绝
                </Button>
                <Button
                  appearance="primary"
                  onClick={handleAcceptPairing}
                  disabled={pairProcessing}
                >
                  {pairProcessing ? <Spinner size="tiny" /> : '接受'}
                </Button>
              </DialogActions>
            </DialogBody>
          </DialogSurface>
        </Dialog>
      )}

      {/* Data Received Import Dialog */}
      {receivedData && (
        <Dialog open={true}>
          <DialogSurface style={{ maxWidth: 400 }}>
            <DialogBody>
              <DialogTitle>接收数据导入</DialogTitle>
              <DialogContent>
                <div style={{ display: 'grid', gap: 12 }}>
                  <Text size={300}>
                    已接收到来自设备的数据
                  </Text>
                  <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
                    会话: {receivedData.session_id}
                  </Text>
                  <div>
                    <Label>导入到身份</Label>
                    <Select
                      value={importIdentityId}
                      onChange={(e) => setImportIdentityId(e.currentTarget.value)}
                      style={{ width: '100%' }}
                    >
                      {identities.map((id) => (
                        <option key={id.id} value={id.id.toString()}>{id.name}</option>
                      ))}
                    </Select>
                  </div>
                </div>
              </DialogContent>
              <DialogActions>
                <Button
                  appearance="secondary"
                  onClick={() => setReceivedData(null)}
                  disabled={importing}
                >
                  取消
                </Button>
                <Button
                  appearance="primary"
                  onClick={handleImportReceivedData}
                  disabled={importing || !importIdentityId}
                >
                  {importing ? <Spinner size="tiny" /> : '导入'}
                </Button>
              </DialogActions>
            </DialogBody>
          </DialogSurface>
        </Dialog>
      )}
    </>
  );
};
