import React, { useState, useEffect } from 'react';
import { Button, Input, Switch, Text, MessageBar, MessageBarBody } from '@fluentui/react-components';
import { ArrowSync24Regular } from '@fluentui/react-icons';
import type { WebDavConfig, BackupMeta, CloudBackupAutoConfig } from '../../types';
import * as tauri from '../../services/tauri';

export const CloudBackupSettings: React.FC<{ onMessage: (msg: string) => void }> = ({ onMessage }) => {
  const [config, setConfig] = useState<WebDavConfig>({ server_url: '', username: '', password: '', backup_root: 'shmtu-backup' });
  const [passInput, setPassInput] = useState('');
  const [testResult, setTestResult] = useState('');
  const [testing, setTesting] = useState(false);
  const [auto, setAuto] = useState<CloudBackupAutoConfig>({ auto_enabled: false, auto_interval_minutes: 360, max_keep: 10 });
  const [backupPwd, setBackupPwd] = useState('');
  const [restorePwd, setRestorePwd] = useState('');
  const [remoteList, setRemoteList] = useState<BackupMeta[]>([]);
  const [backing, setBacking] = useState(false);
  const [backupMsg, setBackupMsg] = useState('');

  useEffect(() => { void load(); }, []);
  const load = async () => {
    try { setConfig(await tauri.cloud_backup_get_config()); } catch (_) {}
    try { setAuto(await tauri.cloud_backup_get_auto_config()); } catch (_) {}
    await refreshRemote();
  };
  const save = async () => {
    await tauri.cloud_backup_save_config({ ...config, password: passInput || config.password });
    onMessage('WebDAV 配置已保存');
  };
  const test = async () => {
    await tauri.cloud_backup_save_config({ ...config, password: passInput || config.password });
    setTesting(true); setTestResult('');
    try {
      const ok = await tauri.cloud_backup_test_connection();
      if (!ok) { setTestResult('✗ 连接失败'); setTesting(false); return; }
      setTestResult(await tauri.cloud_backup_test_write_read());
    } catch (e) { setTestResult(`✗ ${e}`); }
    setTesting(false);
  };
  const refreshRemote = async () => {
    try { setRemoteList(await tauri.cloud_backup_list_remote()); } catch (_) {}
  };

  return (
    <div style={{ display: 'grid', gap: 14 }}>
      <Text weight="semibold" size={400}>云备份</Text>
      <div style={{ display: 'grid', gap: 8 }}>
        <Text weight="semibold" size={300}>WebDAV 配置</Text>
        <Input placeholder="服务器地址 https://dav.example.com" value={config.server_url} onChange={(_, d) => setConfig({ ...config, server_url: d.value })} />
        <Input placeholder="用户名" value={config.username} onChange={(_, d) => setConfig({ ...config, username: d.value })} />
        <Input type="password" placeholder="密码" value={passInput} onChange={(_, d) => setPassInput(d.value)} />
        <Input placeholder="远端根目录" value={config.backup_root} onChange={(_, d) => setConfig({ ...config, backup_root: d.value || 'shmtu-backup' })} />
        <div style={{ display: 'flex', gap: 8 }}>
          <Button appearance="primary" onClick={test} disabled={testing}>{testing ? '测试中...' : '测试连接'}</Button>
          <Button appearance="secondary" onClick={save}>保存</Button>
        </div>
        {testResult && <MessageBar intent={testResult.startsWith('✓') ? 'success' : 'error'}><MessageBarBody>{testResult}</MessageBarBody></MessageBar>}
      </div>
      <div style={{ display: 'grid', gap: 8 }}>
        <Text weight="semibold" size={300}>自动备份</Text>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
          <Text>定时自动备份</Text>
          <Switch checked={auto.auto_enabled} onChange={(_, d) => { setAuto({ ...auto, auto_enabled: d.checked }); tauri.cloud_backup_set_auto_enabled(d.checked); }} />
        </div>
        {auto.auto_enabled && (
          <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
            <Text style={{ whiteSpace: 'nowrap' }}>间隔(分):</Text>
            <Input type="number" min={15} value={String(auto.auto_interval_minutes)}
              onChange={(_, d) => { const v = parseInt(d.value) || 360; setAuto({ ...auto, auto_interval_minutes: v }); tauri.cloud_backup_set_auto_interval(v); }} style={{ width: 100 }} />
          </div>
        )}
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <Text style={{ whiteSpace: 'nowrap' }}>最大保留:</Text>
          <Input type="number" min={1} max={100} value={String(auto.max_keep)}
            onChange={(_, d) => { const v = parseInt(d.value) || 10; setAuto({ ...auto, max_keep: v }); tauri.cloud_backup_set_max_keep(v); }} style={{ width: 80 }} />
        </div>
      </div>
      <div style={{ display: 'grid', gap: 8 }}>
        <Text weight="semibold" size={300}>立即备份</Text>
        <Input type="password" placeholder="加密密码(留空不加密)" value={backupPwd} onChange={(_, d) => setBackupPwd(d.value)} />
        <Button appearance="primary" onClick={async () => {
          setBacking(true); try { setBackupMsg(await tauri.cloud_backup_now(backupPwd || undefined)); await refreshRemote(); }
          catch (e) { setBackupMsg(`✗ ${e}`); } setBacking(false);
        }} disabled={backing}>{backing ? '备份中...' : '立即备份'}</Button>
        {backupMsg && <MessageBar intent={backupMsg.startsWith('✓') ? 'success' : 'error'}><MessageBarBody>{backupMsg}</MessageBarBody></MessageBar>}
      </div>
      <div style={{ display: 'grid', gap: 8 }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
          <Text weight="semibold" size={300}>远程备份 ({remoteList.length})</Text>
          <Button appearance="subtle" icon={<ArrowSync24Regular />} onClick={refreshRemote}>刷新</Button>
        </div>
        {remoteList.length === 0 ? <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>暂无备份</Text> :
          remoteList.map(meta => {
            const isEnc = meta.name.endsWith('.enc');
            const sz = meta.size >= 1048576 ? `${(meta.size / 1048576).toFixed(1)} MB` : meta.size >= 1024 ? `${(meta.size / 1024).toFixed(1)} KB` : `${meta.size} B`;
            return (
              <div key={meta.remote_path} style={{ padding: 10, borderRadius: 8, border: '1px solid var(--colorNeutralStroke2)', display: 'grid', gap: 6 }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                  <div><Text>{meta.name}</Text><Text size={100} block style={{ color: 'var(--colorNeutralForeground3)' }}>{sz}{isEnc ? ' · 🔒' : ''}</Text></div>
                  <div style={{ display: 'flex', gap: 4 }}>
                    <Button size="small" appearance="subtle" onClick={async () => {
                      try { const r = await tauri.cloud_backup_restore(meta.remote_path, restorePwd || undefined); onMessage(`✓ 恢复: ${r.identity_count}身份/${r.account_count}账号/${r.bill_count}账单`); }
                      catch (e) { onMessage(`✗ ${e}`); }
                    }}>恢复</Button>
                    <Button size="small" appearance="subtle" onClick={async () => { await tauri.cloud_backup_delete_remote(meta.remote_path); await refreshRemote(); }}>删除</Button>
                  </div>
                </div>
                {isEnc && <Input placeholder="解密密码" type="password" size="small" value={restorePwd} onChange={(_, d) => setRestorePwd(d.value)} />}
              </div>
            );
          })}
      </div>
    </div>
  );
};
