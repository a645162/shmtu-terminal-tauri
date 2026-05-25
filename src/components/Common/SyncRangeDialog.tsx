import React, { useState } from 'react';
import {
  Button,
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  Field,
  Radio,
  RadioGroup,
  Text,
} from '@fluentui/react-components';
import { CalendarDataBar24Regular } from '@fluentui/react-icons';
import { useAppStore } from '../../stores/appStore';
import type { SyncRangePreset } from '../../types';

const RANGE_OPTIONS: Array<{ value: SyncRangePreset; label: string; description: string }> = [
  { value: 'week', label: '最近一周', description: '只同步最近 7 天账单，最快。' },
  { value: 'half_month', label: '最近半个月', description: '同步最近 15 天账单。' },
  { value: 'month', label: '最近一个月', description: '同步最近 30 天账单，适合常规补账。' },
  { value: 'half_year', label: '最近半年', description: '同步最近 6 个月账单。' },
  { value: 'year', label: '最近一年', description: '同步最近 1 年账单。' },
  { value: 'all', label: '全部', description: '不设时间限制，完整抓取可访问账单。' },
];

function getActionLabel(action: ReturnType<typeof useAppStore.getState>['pendingSyncAction']) {
  if (!action) return '同步';
  switch (action.kind) {
    case 'identity_incremental':
      return '增量更新当前身份';
    case 'identity_full':
      return '全量更新当前身份';
    case 'account_incremental':
      return `增量更新账号 ${action.accountId}`;
    case 'account_full':
      return `全量更新账号 ${action.accountId}`;
  }
}

export const SyncRangeDialog: React.FC = () => {
  const showSyncRangeDialog = useAppStore((s) => s.showSyncRangeDialog);
  const pendingSyncAction = useAppStore((s) => s.pendingSyncAction);
  const closeSyncRangeDialog = useAppStore((s) => s.closeSyncRangeDialog);
  const confirmSyncRange = useAppStore((s) => s.confirmSyncRange);
  const [selectedRange, setSelectedRange] = useState<SyncRangePreset>('month');

  if (!showSyncRangeDialog || !pendingSyncAction) {
    return null;
  }

  return (
    <Dialog open>
      <DialogSurface style={{ width: 'min(92vw, 520px)' }}>
        <DialogBody>
          <DialogTitle>
            <CalendarDataBar24Regular style={{ marginRight: 8 }} />
            选择同步范围
          </DialogTitle>
          <DialogContent>
            <Text block style={{ marginBottom: 16 }}>
              {getActionLabel(pendingSyncAction)}前，请先确认需要抓取的时间范围。
            </Text>

            <Field label="同步范围">
              <RadioGroup
                value={selectedRange}
                onChange={(_, data) => setSelectedRange(data.value as SyncRangePreset)}
              >
                {RANGE_OPTIONS.map((option) => (
                  <Radio
                    key={option.value}
                    value={option.value}
                    label={
                      <div>
                        <div>{option.label}</div>
                        <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
                          {option.description}
                        </Text>
                      </div>
                    }
                  />
                ))}
              </RadioGroup>
            </Field>
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" onClick={closeSyncRangeDialog}>
              取消
            </Button>
            <Button appearance="primary" onClick={() => void confirmSyncRange(selectedRange)}>
              开始同步
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
