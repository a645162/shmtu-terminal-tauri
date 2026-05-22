import React, { useState, useMemo } from 'react';
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
  Text,
  Subtitle2,
  Card,
  CardHeader,
  Divider,
  Spinner,
  Title3,
} from '@fluentui/react-components';
import { ChartMultiple24Regular } from '@fluentui/react-icons';
import { useAppStore } from '../../stores/appStore';
import { ExpenseTrendChart } from '../../components/Charts/ExpenseTrendChart';
import { CategoryPieChart } from '../../components/Charts/CategoryPieChart';
import { MealDistChart } from '../../components/Charts/MealDistChart';
import type { StatisticsSummary } from '../../types';

// Mock summary
const MOCK_SUMMARY: StatisticsSummary = {
  total_expense: 680,
  total_income: 500,
  net_expense: 180,
  daily_average: 22.67,
  expense_count: 89,
  income_count: 2,
};

export const StatisticsDialog: React.FC = () => {
  const showStatisticsDialog = useAppStore((s) => s.showStatisticsDialog);
  const setShowStatisticsDialog = useAppStore((s) => s.setShowStatisticsDialog);
  const identities = useAppStore((s) => s.identities);
  const currentIdentity = useAppStore((s) => s.currentIdentity);
  const theme = useAppStore((s) => s.theme);

  const [selectedIdentityId, setSelectedIdentityId] = useState(
    currentIdentity?.id?.toString() ?? ''
  );
  const [dateRange, setDateRange] = useState('month');

  const summary = MOCK_SUMMARY; // Will be fetched from backend when available

  return (
    <Dialog open={showStatisticsDialog} onOpenChange={(_, data) => !data.open && setShowStatisticsDialog(false)}>
      <DialogSurface style={{ maxWidth: 800, width: '90vw' }}>
        <DialogBody>
          <DialogTitle>
            <ChartMultiple24Regular style={{ marginRight: 8 }} />
            统计分析
          </DialogTitle>
          <DialogContent>
            {/* Filters */}
            <div style={{ display: 'flex', gap: 8, marginBottom: 16 }}>
              <Dropdown
                value={dateRange === 'week' ? '本周' : dateRange === 'month' ? '本月' : '近30天'}
                selectedOptions={[dateRange]}
                onOptionSelect={(_, data) => setDateRange(data.optionValue ?? 'month')}
                style={{ minWidth: 120 }}
              >
                <Option value="week">本周</Option>
                <Option value="month">本月</Option>
                <Option value="30days">近30天</Option>
              </Dropdown>
              <Dropdown
                value={identities.find((i) => i.id.toString() === selectedIdentityId)?.name ?? ''}
                selectedOptions={[selectedIdentityId]}
                onOptionSelect={(_, data) => setSelectedIdentityId(data.optionValue ?? '')}
                style={{ minWidth: 120 }}
              >
                {identities.map((i) => (
                  <Option key={i.id} value={i.id.toString()}>
                    {i.name}
                  </Option>
                ))}
              </Dropdown>
            </div>

            {/* Charts */}
            <Card style={{ padding: 16, marginBottom: 12 }}>
              <CardHeader>
                <Subtitle2>消费趋势</Subtitle2>
              </CardHeader>
              <ExpenseTrendChart />
            </Card>

            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12, marginBottom: 12 }}>
              <Card style={{ padding: 16 }}>
                <CardHeader>
                  <Subtitle2>消费分类占比</Subtitle2>
                </CardHeader>
                <CategoryPieChart />
              </Card>
              <Card style={{ padding: 16 }}>
                <CardHeader>
                  <Subtitle2>用餐时段分布</Subtitle2>
                </CardHeader>
                <MealDistChart />
              </Card>
            </div>

            {/* Summary */}
            <Card style={{ padding: 16 }}>
              <CardHeader>
                <Subtitle2>统计摘要</Subtitle2>
              </CardHeader>
              <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: 16, marginTop: 8 }}>
                <SummaryItem label="总消费" value={`¥${summary.total_expense.toFixed(2)}`} color="var(--colorPaletteRedForeground3)" />
                <SummaryItem label="总充值" value={`¥${summary.total_income.toFixed(2)}`} color="var(--colorPaletteGreenForeground3)" />
                <SummaryItem label="净支出" value={`¥${summary.net_expense.toFixed(2)}`} color="var(--colorBrandForeground1)" />
                <SummaryItem label="日均消费" value={`¥${summary.daily_average.toFixed(2)}`} />
                <SummaryItem label="消费笔数" value={`${summary.expense_count}笔`} />
                <SummaryItem label="充值笔数" value={`${summary.income_count}笔`} />
              </div>
            </Card>
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" onClick={() => setShowStatisticsDialog(false)}>
              关闭
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
};

const SummaryItem: React.FC<{
  label: string;
  value: string;
  color?: string;
}> = ({ label, value, color }) => (
  <div>
    <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }} block>
      {label}
    </Text>
    <Title3 style={{ color: color ?? 'inherit' }}>{value}</Title3>
  </div>
);
