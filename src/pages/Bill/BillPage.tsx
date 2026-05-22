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
  Dialog,
  DialogSurface,
  DialogTitle,
  DialogBody,
  DialogContent,
  DialogActions,
} from '@fluentui/react-components';
import {
  ArrowSync24Regular,
  Search24Regular,
  Delete24Regular,
  Copy24Regular,
  Info24Regular,
  MoreVertical24Regular,
  ChevronLeft24Regular,
  ChevronRight24Regular,
} from '@fluentui/react-icons';
import { useAppStore } from '../../stores/appStore';
import type { BillItem, BillType } from '../../types';
import { formatMoney } from '../../hooks';

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

export const BillPage: React.FC = () => {
  const bills = useAppStore((s) => s.bills);
  const billTotal = useAppStore((s) => s.billTotal);
  const billPage = useAppStore((s) => s.billPage);
  const billPageSize = useAppStore((s) => s.billPageSize);
  const billType = useAppStore((s) => s.billType);
  const billKeyword = useAppStore((s) => s.billKeyword);
  const isLoading = useAppStore((s) => s.isLoading);
  const currentIdentity = useAppStore((s) => s.currentIdentity);
  const setBillPage = useAppStore((s) => s.setBillPage);
  const setBillType = useAppStore((s) => s.setBillType);
  const setBillKeyword = useAppStore((s) => s.setBillKeyword);
  const loadBills = useAppStore((s) => s.loadBills);
  const startSync = useAppStore((s) => s.startSync);

  const [searchInput, setSearchInput] = useState(billKeyword);
  const [dateRange, setDateRange] = useState('all');
  const [detailBill, setDetailBill] = useState<BillItem | null>(null);

  const totalPages = Math.max(1, Math.ceil(billTotal / billPageSize));

  const handleSearch = useCallback(() => {
    setBillKeyword(searchInput);
    loadBills();
  }, [searchInput, setBillKeyword, loadBills]);

  const handleSync = useCallback(() => {
    if (currentIdentity) {
      startSync(currentIdentity.id);
    }
  }, [currentIdentity, startSync]);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
      {/* Filter Bar */}
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
          onOptionSelect={(_, data) => setDateRange(data.optionValue ?? 'all')}
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
          contentAfter={<Search24Regular />}
          style={{ minWidth: 240 }}
        />

        <Button appearance="primary" icon={<ArrowSync24Regular />} onClick={handleSync}>
          同步
        </Button>

        <Text size={200} style={{ marginLeft: 'auto', color: 'var(--colorNeutralForeground3)' }}>
          共 {billTotal} 条
        </Text>
      </div>

      {/* Table */}
      <div style={{ flex: 1, overflow: 'auto' }}>
        {isLoading ? (
          <div style={{ display: 'flex', justifyContent: 'center', padding: 48 }}>
            <Spinner size="large" label="加载中..." />
          </div>
        ) : bills.length === 0 ? (
          <div style={{ textAlign: 'center', padding: 48, color: 'var(--colorNeutralForeground3)' }}>
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
                <TableHeaderCell style={{ minWidth: 100 }}>金额</TableHeaderCell>
                <TableHeaderCell style={{ minWidth: 100 }}>支付方式</TableHeaderCell>
                <TableHeaderCell style={{ minWidth: 80 }}>状态</TableHeaderCell>
                <TableHeaderCell style={{ width: 40 }}></TableHeaderCell>
              </TableRow>
            </TableHeader>
            <TableBody>
              {bills.map((item) => (
                <TableRow key={item.id}>
                  <TableCell>
                    <Text size={200}>{item.date_time_formatted || `${item.date_str} ${item.time_str_formatted}`}</Text>
                  </TableCell>
                  <TableCell>
                    <TableCellLayout>
                      <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
                        <Text size={200}>{item.item_type}</Text>
                        {item.is_combined && <Badge appearance="outline" size="small">合并</Badge>}
                      </div>
                    </TableCellLayout>
                  </TableCell>
                  <TableCell>
                    <Text size={200}>{item.target_user || '—'}</Text>
                  </TableCell>
                  <TableCell>
                    <Text
                      size={200}
                      weight="semibold"
                      style={{ color: item.money >= 0 ? 'var(--colorPaletteGreenForeground3)' : 'var(--colorPaletteRedForeground3)' }}
                    >
                      {formatMoney(item.money)}
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
                          <MenuItem icon={<Copy24Regular />} onClick={() => navigator.clipboard.writeText(item.number)}>
                            复制交易号
                          </MenuItem>
                          <MenuItem icon={<Copy24Regular />} onClick={() => navigator.clipboard.writeText(item.money_str)}>
                            复制金额
                          </MenuItem>
                          <MenuItem icon={<Info24Regular />} onClick={() => setDetailBill(item)}>
                            查看详情
                          </MenuItem>
                          <MenuItem icon={<Delete24Regular />}>删除</MenuItem>
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
const BillDetail: React.FC<{ bill: BillItem }> = ({ bill }) => {
  const fields = [
    { label: '日期时间', value: bill.date_time_formatted },
    { label: '交易名称', value: bill.item_type },
    { label: '交易号', value: bill.number },
    { label: '对方账户', value: bill.target_user || '—' },
    { label: '金额', value: formatMoney(bill.money) },
    { label: '支付方式', value: bill.method },
    { label: '状态', value: bill.status_str },
    { label: '是否合并', value: bill.is_combined ? '是' : '否' },
    { label: '来源学号', value: bill.source_account_id || '—' },
    { label: '同步时间', value: bill.synced_at || '—' },
  ];

  return (
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
  );
};
