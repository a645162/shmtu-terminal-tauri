import React, { useState, useEffect, useCallback } from 'react';
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
  Radio,
  RadioGroup,
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
} from '@fluentui/react-components';
import { useAppStore } from '../../stores/appStore';
import type { ExportFormat, SnapshotInfo } from '../../types';
import * as tauri from '../../services/tauri';
import { formatBytes } from '../../hooks';
import {
  PageEnterMotion,
  SectionEnterMotion,
} from '../../components/Common/motion';

type DataTab = 'export' | 'import' | 'snapshot';

export const DataTransferDialog: React.FC = () => {
  const showDataTransferDialog = useAppStore((s) => s.showDataTransferDialog);
  const setShowDataTransferDialog = useAppStore((s) => s.setShowDataTransferDialog);
  const identities = useAppStore((s) => s.identities);
  const currentIdentity = useAppStore((s) => s.currentIdentity);

  const [selectedTab, setSelectedTab] = useState<DataTab>('export');

  // Export state
  const [exportSource, setExportSource] = useState<'original' | 'merged'>('merged');
  const [exportIdentityId, setExportIdentityId] = useState(currentIdentity?.id?.toString() ?? '');
  const [exportFormat, setExportFormat] = useState<ExportFormat>('csv');
  const [exportPath, setExportPath] = useState('');
  const [exporting, setExporting] = useState(false);
  const [exportResult, setExportResult] = useState('');

  // Import state
  const [importPath, setImportPath] = useState('');
  const [importIdentityId, setImportIdentityId] = useState(currentIdentity?.id?.toString() ?? '');
  const [importing, setImporting] = useState(false);
  const [importResult, setImportResult] = useState('');

  // Snapshot state
  const [snapshots, setSnapshots] = useState<SnapshotInfo[]>([]);
  const [loadingSnapshots, setLoadingSnapshots] = useState(false);
  const [creatingSnapshot, setCreatingSnapshot] = useState(false);

  const loadSnapshots = useCallback(async () => {
    setLoadingSnapshots(true);
    try {
      const list = await tauri.list_snapshots();
      setSnapshots(list);
    } catch {
      setSnapshots([]);
    } finally {
      setLoadingSnapshots(false);
    }
  }, []);

  // Load snapshots when switching to snapshot tab
  useEffect(() => {
    if (selectedTab === 'snapshot' && showDataTransferDialog) {
      loadSnapshots();
    }
  }, [selectedTab, showDataTransferDialog, loadSnapshots]);

  const handleExport = async () => {
    setExporting(true);
    setExportResult('');
    try {
      const result = await tauri.export_data({
        identityId: parseInt(exportIdentityId),
        format: exportFormat,
        sourceType: exportSource,
        filePath: exportPath || `./Data/export/bills_${Date.now()}.${exportFormat}`,
      });
      setExportResult(`导出成功: ${result}`);
    } catch (e) {
      setExportResult('导出失败');
    } finally {
      setExporting(false);
    }
  };

  const handleImport = async () => {
    setImporting(true);
    setImportResult('');
    try {
      const count = await tauri.import_data(importPath, parseInt(importIdentityId));
      setImportResult(`导入成功: ${count} 条记录`);
    } catch {
      setImportResult('导入失败');
    } finally {
      setImporting(false);
    }
  };

  const handleCreateSnapshot = async () => {
    setCreatingSnapshot(true);
    try {
      await tauri.create_snapshot();
      await loadSnapshots();
    } catch {
      // ignore
    } finally {
      setCreatingSnapshot(false);
    }
  };

  const handleRestoreSnapshot = async (filename: string) => {
    if (!confirm('恢复快照将覆盖当前数据，确定继续吗？')) return;
    try {
      await tauri.restore_snapshot(filename);
      await loadSnapshots();
    } catch {
      // ignore
    }
  };

  const renderContent = () => {
    switch (selectedTab) {
      case 'export':
        return (
          <div style={{ display: 'grid', gap: 12 }}>
            <Text weight="semibold">数据导出</Text>
            <div>
              <Label>数据范围</Label>
              <Dropdown
                value={exportSource === 'merged' ? '身份合并数据' : '账号原始数据'}
                selectedOptions={[exportSource]}
                onOptionSelect={(_, data) => setExportSource(data.optionValue as 'original' | 'merged')}
                style={{ width: '100%' }}
              >
                <Option value="merged">身份合并数据</Option>
                <Option value="original">账号原始数据</Option>
              </Dropdown>
            </div>
            <div>
              <Label>身份选择</Label>
              <Dropdown
                value={identities.find((i) => i.id.toString() === exportIdentityId)?.name ?? ''}
                selectedOptions={[exportIdentityId]}
                onOptionSelect={(_, data) => setExportIdentityId(data.optionValue ?? '')}
                style={{ width: '100%' }}
              >
                {identities.map((i) => (
                  <Option key={i.id} value={i.id.toString()}>
                    {i.name}
                  </Option>
                ))}
              </Dropdown>
            </div>
            <div>
              <Label>导出格式</Label>
              <RadioGroup
                value={exportFormat}
                onChange={(_, data) => setExportFormat(data.value as ExportFormat)}
                layout="horizontal"
              >
                <Radio value="csv" label="CSV" />
                <Radio value="json" label="JSON" />
                <Radio value="qianji" label="钱迹格式" />
              </RadioGroup>
            </div>
            <div>
              <Label>保存路径</Label>
              <Input
                value={exportPath}
                onChange={(e) => setExportPath(e.currentTarget.value)}
                placeholder="留空则使用默认路径"
                style={{ width: '100%' }}
              />
            </div>
            {exportResult && (
              <MessageBar intent={exportResult.includes('成功') ? 'success' : 'error'}>
                <MessageBarBody>{exportResult}</MessageBarBody>
              </MessageBar>
            )}
            <Button appearance="primary" onClick={handleExport} disabled={exporting}>
              {exporting ? <Spinner size="tiny" /> : '开始导出'}
            </Button>
          </div>
        );

      case 'import':
        return (
          <div style={{ display: 'grid', gap: 12 }}>
            <Text weight="semibold">数据导入</Text>
            <div>
              <Label>导入文件路径 (JSON)</Label>
              <Input
                value={importPath}
                onChange={(e) => setImportPath(e.currentTarget.value)}
                placeholder="选择JSON文件"
                style={{ width: '100%' }}
              />
            </div>
            <div>
              <Label>目标身份</Label>
              <Dropdown
                value={identities.find((i) => i.id.toString() === importIdentityId)?.name ?? ''}
                selectedOptions={[importIdentityId]}
                onOptionSelect={(_, data) => setImportIdentityId(data.optionValue ?? '')}
                style={{ width: '100%' }}
              >
                {identities.map((i) => (
                  <Option key={i.id} value={i.id.toString()}>
                    {i.name}
                  </Option>
                ))}
              </Dropdown>
            </div>
            {importResult && (
              <MessageBar intent={importResult.includes('成功') ? 'success' : 'error'}>
                <MessageBarBody>{importResult}</MessageBarBody>
              </MessageBar>
            )}
            <Button appearance="primary" onClick={handleImport} disabled={importing}>
              {importing ? <Spinner size="tiny" /> : '开始导入'}
            </Button>
          </div>
        );

      case 'snapshot':
        return (
          <div style={{ display: 'grid', gap: 12 }}>
            <Text weight="semibold">快照管理</Text>
            <div style={{ display: 'flex', gap: 8 }}>
              <Button appearance="primary" onClick={handleCreateSnapshot} disabled={creatingSnapshot}>
                {creatingSnapshot ? <Spinner size="tiny" /> : '创建快照'}
              </Button>
              <Button appearance="secondary" onClick={loadSnapshots}>
                刷新列表
              </Button>
            </div>
            {loadingSnapshots ? (
              <Spinner label="加载中..." />
            ) : snapshots.length === 0 ? (
              <Text style={{ color: 'var(--colorNeutralForeground3)' }}>暂无快照</Text>
            ) : (
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHeaderCell>快照时间</TableHeaderCell>
                    <TableHeaderCell>大小</TableHeaderCell>
                    <TableHeaderCell>操作</TableHeaderCell>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {snapshots.map((snap) => (
                    <TableRow key={snap.filename} className="motion-table-row">
                      <TableCell>{snap.created_at}</TableCell>
                      <TableCell>{formatBytes(snap.size_bytes)}</TableCell>
                      <TableCell>
                        <Button
                          size="small"
                          appearance="subtle"
                          onClick={() => handleRestoreSnapshot(snap.filename)}
                        >
                          恢复
                        </Button>
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            )}
          </div>
        );
    }
  };

  return (
    <Dialog open={showDataTransferDialog} onOpenChange={(_, data) => !data.open && setShowDataTransferDialog(false)}>
      <DialogSurface style={{ maxWidth: 600 }}>
        <DialogBody>
          <DialogTitle>数据管理</DialogTitle>
          <DialogContent>
            <SectionEnterMotion>
              <div>
                <TabList
                  selectedValue={selectedTab}
                  onTabSelect={(_, data) => setSelectedTab(data.value as DataTab)}
                  style={{ marginBottom: 16 }}
                >
                  <Tab value="export">导出</Tab>
                  <Tab value="import">导入</Tab>
                  <Tab value="snapshot">快照</Tab>
                </TabList>
              </div>
            </SectionEnterMotion>
            <PageEnterMotion key={selectedTab}>
              <div>{renderContent()}</div>
            </PageEnterMotion>
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" onClick={() => setShowDataTransferDialog(false)}>
              关闭
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
};
