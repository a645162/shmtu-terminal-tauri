import React, { useCallback, useState } from 'react';
import { Button, Text, Textarea } from '@fluentui/react-components';
import { Edit24Regular } from '@fluentui/react-icons';
import { useAppStore } from '../../stores/appStore';
import type { BillItem } from '../../types';
import * as tauri from '../../services/tauri';
import { formatBillMoney } from '../../hooks';
import { TransactionDetailDialog } from './TransactionDetailDialog';

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

export function buildBillFeedbackString(bill: BillItem): string {
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

export const BillDetailDialog: React.FC<{
  bill: BillItem;
  onClose: () => void;
}> = ({ bill, onClose }) => {
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
    { label: '日期时间', value: bill.date_time_formatted || '—' },
    { label: '交易名称', value: normalizeTransactionName(bill) },
    { label: '交易号', value: normalizeTransactionNumber(bill) },
    { label: '对方账户', value: bill.target_user || '—' },
    { label: '位置', value: bill.position || '—' },
    { label: '房间/窗口', value: bill.room || '—' },
    { label: '金额', value: formatBillMoney(bill.money, bill.item_type || '') },
    { label: '支付方式', value: bill.method || '—' },
    { label: '状态', value: bill.status_str || '—' },
    { label: '是否合并', value: bill.is_combined ? '是' : '否' },
    { label: '来源学号', value: bill.source_account_id || '—' },
    { label: '同步时间', value: bill.synced_at || '—' },
  ];

  return (
    <TransactionDetailDialog
      open
      onClose={onClose}
      fields={fields}
      rawPayload={bill}
      copyText={buildBillFeedbackString(bill)}
      extraContent={
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
      }
    />
  );
};
