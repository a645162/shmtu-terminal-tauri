import React, { useEffect } from 'react';
import {
  Card,
  CardHeader,
  Text,
  Title3,
  Subtitle2,
  Spinner,
} from '@fluentui/react-components';
import {
  Money24Regular,
  SubtractCircle24Regular,
  AddCircle24Regular,
} from '@fluentui/react-icons';
import { useAppStore } from '../../stores/appStore';
import { ExpenseTrendChart } from '../../components/Charts/ExpenseTrendChart';
import { CategoryPieChart } from '../../components/Charts/CategoryPieChart';
import * as tauri from '../../services/tauri';

function getDateRangeParams(rangeKey: string, identityId: number): tauri.StatisticsParams {
  const now = new Date();
  let dateStart: string | undefined;
  let dateEnd: string | undefined;

  const today = now.toISOString().split('T')[0];
  dateEnd = today;

  switch (rangeKey) {
    case 'week': {
      const d = new Date(now);
      d.setDate(d.getDate() - 6);
      dateStart = d.toISOString().split('T')[0];
      break;
    }
    case 'month': {
      const d = new Date(now.getFullYear(), now.getMonth(), 1);
      dateStart = d.toISOString().split('T')[0];
      break;
    }
    case 'today':
    default: {
      dateStart = today;
      break;
    }
  }

  return { identityId, dateStart, dateEnd };
}

export const HomePage: React.FC = () => {
  const currentIdentity = useAppStore((s) => s.currentIdentity);
  const bills = useAppStore((s) => s.bills);
  const statisticsSummary = useAppStore((s) => s.statisticsSummary);
  const dailyTrend = useAppStore((s) => s.dailyTrend);
  const categoryDistribution = useAppStore((s) => s.categoryDistribution);
  const isLoadingStatistics = useAppStore((s) => s.isLoadingStatistics);
  const todaySummary = useAppStore((s) => s.todaySummary);
  const monthSummary = useAppStore((s) => s.monthSummary);
  const loadTodaySummary = useAppStore((s) => s.loadTodaySummary);
  const loadMonthSummary = useAppStore((s) => s.loadMonthSummary);
  const loadDailyTrend = useAppStore((s) => s.loadDailyTrend);
  const loadCategoryDistribution = useAppStore((s) => s.loadCategoryDistribution);

  useEffect(() => {
    if (!currentIdentity) return;
    const todayParams = getDateRangeParams('today', currentIdentity.id);
    const monthParams = getDateRangeParams('month', currentIdentity.id);
    loadTodaySummary(todayParams);
    loadMonthSummary(monthParams);
    loadDailyTrend(todayParams);
    loadCategoryDistribution(todayParams);
  }, [currentIdentity]);

  const recentBills = bills.slice(0, 5);

  if (!currentIdentity) {
    return (
      <div style={{ display: 'flex', justifyContent: 'center', alignItems: 'center', height: '100%', padding: 40 }}>
        <Text size={400} style={{ color: 'var(--colorNeutralForeground3)' }}>
          请先选择一个身份以查看首页
        </Text>
      </div>
    );
  }

  return (
    <div style={{ padding: 20, maxWidth: 1200, margin: '0 auto' }}>
      {/* Summary Cards */}
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))', gap: 12, marginBottom: 20 }}>
        <StatCard
          title="今日消费"
          value={todaySummary ? `¥ ${Math.abs(todaySummary.total_expense).toFixed(2)}` : '加载中...'}
          icon={<SubtractCircle24Regular />}
          color="var(--colorPaletteRedForeground3)"
        />
        <StatCard
          title="本月消费"
          value={todaySummary ? `¥ ${Math.abs(todaySummary.total_expense).toFixed(2)}` : '加载中...'}
          icon={<SubtractCircle24Regular />}
          color="var(--colorPaletteRedForeground3)"
        />
        <StatCard
          title="本月充值"
          value={monthSummary ? `¥ ${monthSummary.total_income.toFixed(2)}` : '加载中...'}
          icon={<AddCircle24Regular />}
          color="var(--colorPaletteGreenForeground3)"
        />
        <StatCard
          title="卡片余额"
          value="暂不可用"
          icon={<Money24Regular />}
          color="var(--colorBrandForeground1)"
        />
      </div>

      {/* Charts Row */}
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12, marginBottom: 20 }}>
        <Card style={{ padding: 16 }}>
          <CardHeader>
            <Subtitle2>近7日消费趋势</Subtitle2>
          </CardHeader>
          {isLoadingStatistics ? (
            <div style={{ display: 'flex', justifyContent: 'center', padding: 40 }}>
              <Spinner label="加载中..." />
            </div>
          ) : (
            <ExpenseTrendChart data={dailyTrend} />
          )}
        </Card>
        <Card style={{ padding: 16 }}>
          <CardHeader>
            <Subtitle2>消费分类占比</Subtitle2>
          </CardHeader>
          <CategoryPieChart data={categoryDistribution} />
        </Card>
      </div>

      {/* Recent Transactions */}
      <Card style={{ padding: 16 }}>
        <CardHeader>
          <Subtitle2>最近交易</Subtitle2>
        </CardHeader>
        {recentBills.length === 0 ? (
          <Text size={200} style={{ color: 'var(--colorNeutralForeground3)', padding: 24, display: 'block', textAlign: 'center' }}>
            暂无交易记录
          </Text>
        ) : (
          <div>
            {recentBills.map((bill) => (
              <div
                key={bill.id}
                style={{
                  display: 'flex',
                  justifyContent: 'space-between',
                  alignItems: 'center',
                  padding: '8px 0',
                  borderBottom: '1px solid var(--colorNeutralStroke2)',
                }}
              >
                <div>
                  <Text block size={200}>{bill.item_type}</Text>
                  <Text size={100} style={{ color: 'var(--colorNeutralForeground3)' }}>
                    {bill.date_time_formatted}
                  </Text>
                </div>
                <Text
                  weight="semibold"
                  style={{ color: bill.money >= 0 ? 'var(--colorPaletteGreenForeground3)' : 'var(--colorPaletteRedForeground3)' }}
                >
                  {bill.money >= 0 ? '+' : ''}{bill.money.toFixed(2)}
                </Text>
              </div>
            ))}
          </div>
        )}
      </Card>
    </div>
  );
};

interface StatCardProps {
  title: string;
  value: string;
  icon: React.ReactNode;
  color: string;
}

const StatCard: React.FC<StatCardProps> = ({ title, value, icon, color }) => (
  <Card style={{ padding: 16 }}>
    <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 8 }}>
      <span style={{ color }}>{icon}</span>
      <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>{title}</Text>
    </div>
    <Title3 block>{value}</Title3>
  </Card>
);
