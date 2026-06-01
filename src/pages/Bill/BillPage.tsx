import React, { useState, useCallback } from 'react';
import {
  Table,
  TableHeader,
  TableHeaderCell,
  TableRow,
  TableCell,
  TableBody,
  TableCellLayout,
  Dropdown,
  Option,
  Input,
  Button,
  Text,
  Badge,
  Spinner,
  Menu,
  MenuTrigger,
  MenuPopover,
  MenuList,
  MenuItem,
  MenuButton,
  Dialog,
  DialogSurface,
  DialogTitle,
  DialogBody,
  DialogContent,
  DialogActions,
  Textarea,
} from '@fluentui/react-components';
import {
  ArrowSync24Regular,
  ArrowClockwise24Regular,
  ArrowDownload24Regular,
  Search24Regular,
  Delete24Regular,
  Copy24Regular,
  Info24Regular,
  MoreVertical24Regular,
  MoreHorizontal24Regular,
  ChevronLeft24Regular,
  ChevronRight24Regular,
  Edit24Regular,
  People24Regular,
  Merge24Regular,
} from '@fluentui/react-icons';
import { useAppStore } from '../../stores/appStore';
import type { BillItem, BillType } from '../../types';
import { formatBillMoney } from '../../hooks';
import * as tauri from '../../services/tauri';
import {
  SectionEnterMotion,
  SlideInFromRightMotion,
} from '../../components/Common/motion';

const BILL_TYPE_OPTIONS: { key: BillType; text: string }[] = [
  { key: 'all', text: '全部' },
  { key: 'success', text: '交易成功' },
  { key: 'not_paid', text: '待支付' },
  { key: 'failure', text: '交易失败' },
];

const DATE_RANGE_OPTIONS = [
  { key: 'all', text: '全部时间' },
  { key: 'today', text: '今天' },
  { key: 'week', text: '本周' },
  { key: 'month', text: '本月' },
];

function normalizeTransactionName(bill: BillItem): string {
  const raw = (bill.item_type || '').replace(/\s+/g, ' ').trim();
  if (!raw) return '—';
  const markerIndex = raw.indexOf('交易号');
  if (markerIndex >= 0) {
    return raw.slice(0, markerIndex).replace(/[：:]\s*$/, '').trim() || '—';
  }
  return raw;
}

function normalizeTransactionNumber(bill: BillItem): string {
  const direct = (bill.number || '').replace(/\D/g, '');
  if (direct) return direct;

  const fromName = ((bill.item_type || '').match(/交易号[：:\s]*([0-9]+)/)?.[1] || '').trim();
  if (fromName) return fromName;

  try {
    const parsed = JSON.parse(bill.number_list || '[]');
    if (Array.isArray(parsed)) {
      const first = parsed
        .map((item) => String(item ?? '').replace(/\D/g, ''))
        .find((item) => item.length > 0);
      if (first) return first;
    }
  } catch {
    // ignore malformed legacy number_list payloads
  }

  return '—';
}

function formatBillLocation(bill: BillItem): string {
  if (bill.position && bill.room && bill.position !== bill.room) {
    return `${bill.position} / ${bill.room}`;
  }
  return bill.room || bill.position || '—';
}

function dateRangeToParams(key: string): { start: string; end: string } {
  const now = new Date();
  const today = now.toISOString().split('T')[0];
  let start: string;
  const end = today;

  switch (key) {
    case 'today':
      start = today;
      break;
    case 'week': {
      const d = new Date(now);
      d.setDate(d.getDate() - 6);
      start = d.toISOString().split('T')[0];
      break;
    }
    case 'month': {
      const d = new Date(now.getFullYear(), now.getMonth(), 1);
      start = d.toISOString().split('T')[0];
      break;
    }
    default:
      start = '';
  }

  return { start, end };
}

export const BillPage: React.FC = () => {
  const bills = useAppStore((s) => s.bills);
  const billTotal = useAppStore((s) => s.billTotal);
  const billPage = useAppStore((s) => s.billPage);
  const billPageSize = useAppStore((s) => s.billPageSize);
  const billType = useAppStore((s) => s.billType);
  const billKeyword = useAppStore((s) => s.billKeyword);
  const isLoading = useAppStore((s) => s.isLoading);
  const currentIdentity = useAppStore((s) => s.currentIdentity);
  const accounts = useAppStore((s) => s.accounts);
  const setBillPage = useAppStore((s) => s.setBillPage);
  const setBillType = useAppStore((s) => s.setBillType);
  const setBillKeyword = useAppStore((s) => s.setBillKeyword);
  const setBillDateRange = useAppStore((s) => s.setBillDateRange);
  const loadBills = useAppStore((s) => s.loadBills);
  const openSyncRangeDialog = useAppStore((s) => s.openSyncRangeDialog);

  const [searchInput, setSearchInput] = useState(billKeyword);
  const [dateRange, setDateRange] = useState('all');
  const [detailBill, setDetailBill] = useState<BillItem | null>(null);
  const [showAccountPanel, setShowAccountPanel] = useState(false);
  const [isRebuilding, setIsRebuilding] = useState(false);

  const totalPages = Math.max(1, Math.ceil(billTotal / billPageSize));

  const handleSearch = useCallback(() => {
    setBillKeyword(searchInput);
    if (currentIdentity) {
      useAppStore.setState({ billKeyword: searchInput, billPage: 1 });
      useAppStore.getState().loadBills();
    }
  }, [searchInput, currentIdentity]);

  const handleDateRangeChange = useCallback((key: string) => {
    setDateRange(key);
    const { start, end } = dateRangeToParams(key);
    setBillDateRange(start, end);
  }, [setBillDateRange]);

  const handleSync = useCallback(() => {
    if (currentIdentity) {
      openSyncRangeDialog({ kind: 'identity_incremental', identityId: currentIdentity.id });
    }
  }, [currentIdentity, openSyncRangeDialog]);

  const handleRefresh = useCallback(() => {
    if (currentIdentity) {
      loadBills();
    }
  }, [currentIdentity, loadBills]);

  const handleFullSync = useCallback(() => {
    if (currentIdentity) {
      openSyncRangeDialog({ kind: 'identity_full', identityId: currentIdentity.id });
    }
  }, [currentIdentity, openSyncRangeDialog]);

  const handleAccountSync = useCallback((accountId: string) => {
    if (currentIdentity) {
      openSyncRangeDialog({
        kind: 'account_incremental',
        identityId: currentIdentity.id,
        accountId,
      });
    }
  }, [currentIdentity, openSyncRangeDialog]);

  const handleAccountFullSync = useCallback((accountId: string) => {
    if (currentIdentity) {
      openSyncRangeDialog({
        kind: 'account_full',
        identityId: currentIdentity.id,
        accountId,
      });
    }
  }, [currentIdentity, openSyncRangeDialog]);

  const handleDeleteBill = useCallback(
    async (bill: BillItem) => {
      if (!currentIdentity) return;
      if (!confirm('确定要删除这条账单记录吗？')) return;
      try {
        await tauri.delete_merged_bill(currentIdentity.id, bill.id);
        loadBills();
      } catch (e) {
        console.error('Failed to delete bill:', e);
      }
    },
    [currentIdentity, loadBills]
  );

  const handleRebuildMerged = useCallback(async () => {
    if (!currentIdentity) return;
    if (!confirm('重新合并将清空当前合并表并从原始数据重建，确定继续吗？')) return;
    setIsRebuilding(true);
    try {
      const count = await tauri.rebuild_merged_bills(currentIdentity.id);
      alert(`重新合并完成，共重建 ${count} 条记录`);
      loadBills();
    } catch (e) {
      console.error('Failed to rebuild merged bills:', e);
      alert(`重新合并失败: ${e}`);
    } finally {
      setIsRebuilding(false);
    }
  }, [currentIdentity, loadBills]);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
      {/* Filter Bar */}
      <SectionEnterMotion>
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: 8,
            padding: '12px 20px',
            borderBottom: '1px solid var(--colorNeutralStroke2)',
            flexWrap: 'wrap',
            flexShrink: 0,
          }}
        >
          <Dropdown
            value={BILL_TYPE_OPTIONS.find((o) => o.key === billType)?.text ?? '全部'}
            selectedOptions={[billType]}
            onOptionSelect={(_, data) => setBillType(data.optionValue as BillType)}
            style={{ minWidth: 120 }}
          >
            {BILL_TYPE_OPTIONS.map((opt) => (
              <Option key={opt.key} value={opt.key}>
                {opt.text}
              </Option>
            ))}
          </Dropdown>

          <Dropdown
            value={DATE_RANGE_OPTIONS.find((o) => o.key === dateRange)?.text ?? '全部时间'}
            selectedOptions={[dateRange]}
            onOptionSelect={(_, data) => handleDateRangeChange(data.optionValue ?? 'all')}
            style={{ minWidth: 120 }}
          >
            {DATE_RANGE_OPTIONS.map((opt) => (
              <Option key={opt.key} value={opt.key}>
                {opt.text}
              </Option>
            ))}
          </Dropdown>

          <Input
            placeholder="搜索交易名称、对方账户..."
            value={searchInput}
            onChange={(e) => setSearchInput(e.currentTarget.value)}
            onKeyDown={(e) => e.key === 'Enter' && handleSearch()}
            contentAfter={
              <Button
                appearance="subtle"
                icon={<Search24Regular />}
                onClick={handleSearch}
                size="small"
                style={{ pointerEvents: 'auto' }}
              />
            }
            style={{ minWidth: 240 }}
          />

          <div style={{ marginLeft: 'auto', display: 'flex', alignItems: 'center', gap: 8 }}>
            <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
              共 {billTotal} 条
            </Text>
            <Button
              appearance="secondary"
              icon={
                <span className={isLoading ? 'motion-spin-indefinite' : undefined}>
                  <ArrowClockwise24Regular />
                </span>
              }
              onClick={handleRefresh}
            >
              刷新
            </Button>
            <Menu>
              <MenuTrigger>
                <MenuButton appearance="primary" icon={<MoreHorizontal24Regular />}>
                  更多操作
                </MenuButton>
              </MenuTrigger>
              <MenuPopover>
                <MenuList>
                  <MenuItem icon={<ArrowSync24Regular />} onClick={handleSync}>
                    增量更新（全部账号）
                  </MenuItem>
                  <MenuItem icon={<ArrowDownload24Regular />} onClick={handleFullSync}>
                    全量更新（全部账号）
                  </MenuItem>
                  <MenuItem icon={<ArrowClockwise24Regular />} onClick={handleRefresh}>
                    刷新数据
                  </MenuItem>
                  <MenuItem icon={<Merge24Regular />} onClick={handleRebuildMerged} disabled={isRebuilding}>
                    {isRebuilding ? '重新合并中...' : '重新合并'}
                  </MenuItem>
                </MenuList>
              </MenuPopover>
            </Menu>
            {accounts.length > 0 && (
              <Button
                appearance="subtle"
                icon={<People24Regular />}
                onClick={() => setShowAccountPanel(!showAccountPanel)}
              >
                {showAccountPanel ? '隐藏账号' : `账号 (${accounts.length})`}
              </Button>
            )}
          </div>
        </div>
      </SectionEnterMotion>

      {/* Account Sync Panel */}
      {showAccountPanel && (
        <SectionEnterMotion>
          <div
            style={{
              padding: '8px 20px',
              borderBottom: '1px solid var(--colorNeutralStroke2)',
              backgroundColor: 'var(--colorNeutralBackground3)',
            }}
          >
            <Text size={200} weight="semibold" style={{ marginBottom: 8, display: 'block' }}>
              账号级别同步
            </Text>
            <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
              {accounts.map((account) => (
                <div
                  key={account.id}
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'space-between',
                    padding: '6px 12px',
                    borderRadius: 6,
                    backgroundColor: 'var(--colorNeutralBackground1)',
                    border: '1px solid var(--colorNeutralStroke2)',
                  }}
                >
                  <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
                    <Text size={200} weight="semibold">{account.account_name}</Text>
                    <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
                      {account.account_id}
                    </Text>
                    {account.enable && account.enable_update ? (
                      <Badge appearance="filled" color="success" size="small">启用</Badge>
                    ) : (
                      <Badge appearance="outline" size="small">禁用</Badge>
                    )}
                  </div>
                  <div style={{ display: 'flex', gap: 4 }}>
                    <Button
                      appearance="subtle"
                      size="small"
                      icon={<ArrowSync24Regular />}
                      onClick={() => handleAccountSync(account.account_id)}
                      disabled={!account.enable || !account.enable_update}
                    >
                      增量
                    </Button>
                    <Button
                      appearance="subtle"
                      size="small"
                      icon={<ArrowDownload24Regular />}
                      onClick={() => handleAccountFullSync(account.account_id)}
                      disabled={!account.enable || !account.enable_update}
                    >
                      全量
                    </Button>
                  </div>
                </div>
              ))}
            </div>
          </div>
        </SectionEnterMotion>
      )}

      {/* Table */}
      <div style={{ flex: 1, overflow: 'auto' }}>
        <SlideInFromRightMotion delay={70}>
          <div>
            {isLoading ? (
              <div style={{ display: 'flex', justifyContent: 'center', padding: 48 }}>
                <Spinner size="large" label="加载中..." />
              </div>
            ) : bills.length === 0 ? (
              <div className="motion-empty-state" style={{ textAlign: 'center', padding: 48, color: 'var(--colorNeutralForeground3)' }}>
                <Text size={400}>暂无账单数据</Text>
                <br />
                <Text size={200}>点击"同步"按钮获取最新账单</Text>
              </div>
            ) : (
              <Table style={{ minWidth: 700 }}>
                <TableHeader>
                  <TableRow>
                    <TableHeaderCell style={{ minWidth: 160 }}>日期时间</TableHeaderCell>
                    <TableHeaderCell style={{ minWidth: 120 }}>交易名称</TableHeaderCell>
                    <TableHeaderCell style={{ minWidth: 140 }}>对方账户</TableHeaderCell>
                    <TableHeaderCell style={{ minWidth: 120 }}>位置</TableHeaderCell>
                    <TableHeaderCell style={{ minWidth: 100 }}>金额</TableHeaderCell>
                    <TableHeaderCell style={{ minWidth: 100 }}>支付方式</TableHeaderCell>
                    <TableHeaderCell style={{ minWidth: 80 }}>状态</TableHeaderCell>
                    <TableHeaderCell style={{ width: 40 }}></TableHeaderCell>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {bills.map((item) => (
                    <TableRow key={item.id} className="motion-table-row">
                      <TableCell>
                        <Text size={200}>{item.date_time_formatted || `${item.date_str} ${item.time_str_formatted}`}</Text>
                      </TableCell>
                      <TableCell>
                        <TableCellLayout>
                          <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
                            <Text size={200}>{normalizeTransactionName(item)}</Text>
                            {item.is_combined && <Badge appearance="outline" size="small">合并</Badge>}
                          </div>
                        </TableCellLayout>
                      </TableCell>
                      <TableCell>
                        <Text size={200}>{item.target_user || '—'}</Text>
                      </TableCell>
                      <TableCell>
                        <Text size={200}>{formatBillLocation(item)}</Text>
                      </TableCell>
                      <TableCell>
                        <Text
                          size={200}
                          weight="semibold"
                          style={{ color: item.item_type?.includes('充值') || item.item_type?.includes('冲正') || item.item_type?.includes('退款') ? 'var(--colorPaletteGreenForeground3)' : 'var(--colorPaletteRedForeground3)' }}
                        >
                          {formatBillMoney(item.money, item.item_type || '')}
                        </Text>
                      </TableCell>
                      <TableCell>
                        <Text size={200}>{item.method}</Text>
                      </TableCell>
                      <TableCell>
                        <Badge
                          appearance="filled"
                          color={item.status_str === '交易成功' ? 'success' : item.status_str === '#fail' ? 'danger' : 'informative'}
                        >
                          {item.status_str}
                        </Badge>
                      </TableCell>
                      <TableCell>
                        <Menu>
                          <MenuTrigger>
                            <Button appearance="subtle" icon={<MoreVertical24Regular />} size="small" />
                          </MenuTrigger>
                          <MenuPopover>
                            <MenuList>
                              <MenuItem icon={<Copy24Regular />} onClick={() => navigator.clipboard.writeText(normalizeTransactionNumber(item))}>
                                复制交易号
                              </MenuItem>
                              <MenuItem icon={<Copy24Regular />} onClick={() => navigator.clipboard.writeText(item.money_str)}>
                                复制金额
                              </MenuItem>
                              <MenuItem icon={<Info24Regular />} onClick={() => setDetailBill(item)}>
                                查看详情
                              </MenuItem>
                              <MenuItem icon={<Delete24Regular />} onClick={() => handleDeleteBill(item)}>
                                删除
                              </MenuItem>
                            </MenuList>
                          </MenuPopover>
                        </Menu>
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            )}
          </div>
        </SlideInFromRightMotion>
      </div>

      {/* Pagination */}
      {totalPages > 1 && (
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            padding: '8px 20px',
            borderTop: '1px solid var(--colorNeutralStroke2)',
            flexShrink: 0,
          }}
        >
          <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
            第 {billPage}/{totalPages} 页
          </Text>
          <div style={{ display: 'flex', gap: 4 }}>
            <Button
              appearance="subtle"
              icon={<ChevronLeft24Regular />}
              disabled={billPage <= 1}
              onClick={() => setBillPage(billPage - 1)}
            />
            <Button
              appearance="subtle"
              icon={<ChevronRight24Regular />}
              disabled={billPage >= totalPages}
              onClick={() => setBillPage(billPage + 1)}
            />
          </div>
        </div>
      )}

      {/* Detail Dialog */}
      {detailBill && (
        <Dialog open onOpenChange={(_, data) => !data.open && setDetailBill(null)}>
          <DialogSurface>
            <DialogBody>
              <DialogTitle>交易详情</DialogTitle>
              <DialogContent>
                <BillDetail bill={detailBill} />
              </DialogContent>
              <DialogActions>
                <Button
                  appearance="secondary"
                  icon={<Copy24Regular />}
                  onClick={async () => {
                    const detailString = buildBillFeedbackString(detailBill);
                    await navigator.clipboard.writeText(detailString);
                  }}
                >
                  复制字符串
                </Button>
                <Button
                  appearance="secondary"
                  icon={<Copy24Regular />}
                  onClick={async () => {
                    const payload = JSON.stringify(detailBill, null, 2);
                    await navigator.clipboard.writeText(payload);
                  }}
                >
                  复制JSON
                </Button>
                <Button appearance="secondary" onClick={() => setDetailBill(null)}>
                  关闭
                </Button>
              </DialogActions>
            </DialogBody>
          </DialogSurface>
        </Dialog>
      )}
    </div>
  );
};

// Bill detail view
function buildBillFeedbackString(bill: BillItem): string {
  return [
    `日期时间: ${bill.date_time_formatted || '—'}`,
    `交易名称: ${normalizeTransactionName(bill)}`,
    `交易号: ${normalizeTransactionNumber(bill)}`,
    `对方账户: ${bill.target_user || '—'}`,
    `位置: ${bill.position || '—'}`,
    `房间/窗口: ${bill.room || '—'}`,
    `金额: ${formatBillMoney(bill.money, bill.item_type || '')}`,
    `支付方式: ${bill.method || '—'}`,
    `状态: ${bill.status_str || '—'}`,
    `来源学号: ${bill.source_account_id || '—'}`,
    `同步时间: ${bill.synced_at || '—'}`,
    `备注: ${bill.notes || '—'}`,
  ].join('\n');
}

const BillDetail: React.FC<{ bill: BillItem }> = ({ bill }) => {
  const currentIdentity = useAppStore((s) => s.currentIdentity);
  const loadBills = useAppStore((s) => s.loadBills);
  const [editingNotes, setEditingNotes] = useState(false);
  const [notesValue, setNotesValue] = useState(bill.notes || '');

  const handleSaveNotes = useCallback(async () => {
    if (!currentIdentity) return;
    try {
      await tauri.update_bill_notes(currentIdentity.id, bill.id, notesValue || null);
      setEditingNotes(false);
      loadBills();
    } catch (e) {
      console.error('Failed to update notes:', e);
    }
  }, [currentIdentity, bill.id, notesValue, loadBills]);

  const fields = [
    { label: '日期时间', value: bill.date_time_formatted },
    { label: '交易名称', value: normalizeTransactionName(bill) },
    { label: '交易号', value: normalizeTransactionNumber(bill) },
    { label: '对方账户', value: bill.target_user || '—' },
    { label: '位置', value: bill.position || '—' },
    { label: '房间/窗口', value: bill.room || '—' },
    { label: '金额', value: formatBillMoney(bill.money, bill.item_type || '') },
    { label: '支付方式', value: bill.method },
    { label: '状态', value: bill.status_str },
    { label: '是否合并', value: bill.is_combined ? '是' : '否' },
    { label: '来源学号', value: bill.source_account_id || '—' },
    { label: '同步时间', value: bill.synced_at || '—' },
  ];

  const feedbackPayload = JSON.stringify(bill, null, 2);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
      <div
        style={{
          padding: 10,
          borderRadius: 8,
          background: 'var(--colorNeutralBackground3)',
          border: '1px solid var(--colorNeutralStroke2)',
        }}
      >
        <Text size={100} style={{ color: 'var(--colorNeutralForeground3)', display: 'block', marginBottom: 4 }}>
          JSON 预览
        </Text>
        <Text
          size={100}
          style={{
            fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Consolas, monospace',
            wordBreak: 'break-all',
          }}
        >
          {feedbackPayload}
        </Text>
      </div>
      <div style={{ display: 'grid', gridTemplateColumns: '120px 1fr', gap: '8px 16px' }}>
        {fields.map((f) => (
          <React.Fragment key={f.label}>
            <Text size={200} weight="semibold" style={{ color: 'var(--colorNeutralForeground3)' }}>
              {f.label}
            </Text>
            <Text size={200}>{f.value}</Text>
          </React.Fragment>
        ))}
      </div>
      <div style={{ borderTop: '1px solid var(--colorNeutralStroke2)', paddingTop: 8, marginTop: 4 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 4 }}>
          <Text size={200} weight="semibold" style={{ color: 'var(--colorNeutralForeground3)' }}>
            备注
          </Text>
          {!editingNotes && (
            <Button appearance="subtle" icon={<Edit24Regular />} size="small" onClick={() => { setEditingNotes(true); setNotesValue(bill.notes || ''); }}>
              编辑
            </Button>
          )}
        </div>
        {editingNotes ? (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
            <Textarea
              value={notesValue}
              onChange={(e) => setNotesValue(e.currentTarget.value)}
              placeholder="添加备注..."
              rows={3}
            />
            <div style={{ display: 'flex', gap: 8 }}>
              <Button appearance="primary" size="small" onClick={handleSaveNotes}>保存</Button>
              <Button appearance="secondary" size="small" onClick={() => setEditingNotes(false)}>取消</Button>
            </div>
          </div>
        ) : (
          <Text size={200} style={{ color: bill.notes ? 'inherit' : 'var(--colorNeutralForeground3)' }}>
            {bill.notes || '暂无备注'}
          </Text>
        )}
      </div>
    </div>
  );
};
