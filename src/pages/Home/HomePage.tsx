import React, { useEffect, useState } from 'react';
import {
  Card,
  CardHeader,
  Text,
  Title3,
  Subtitle2,
  Spinner,
  Button,
  InfoLabel,
  TabList,
  Tab,
} from '@fluentui/react-components';
import {
  Money24Regular,
  SubtractCircle24Regular,
  AddCircle24Regular,
  ArrowSync24Regular,
  ArrowExpand24Regular,
} from '@fluentui/react-icons';
import { useAppStore } from '../../stores/appStore';
import { ExpenseTrendChart } from '../../components/Charts/ExpenseTrendChart';
import { CategoryPieChart } from '../../components/Charts/CategoryPieChart';
import { MonthComparisonCard } from '../../components/Charts/MonthComparisonCard';
import { formatBillMoney } from '../../hooks';
import * as tauri from '../../services/tauri';
import { formatLocalDate } from '../../utils/date';
import {
  CardEnterMotion,
  SectionEnterMotion,
  SlideInFromRightMotion,
  getStaggerDelay,
} from '../../components/Common/motion';

function getDateRangeParams(rangeKey: string, identityId: number): tauri.StatisticsParams {
  const now = new Date();
  let dateStart: string | undefined;
  let dateEnd: string | undefined;

  const today = formatLocalDate(now);
  dateEnd = today;

  switch (rangeKey) {
    case 'recent7days': {
      const d = new Date(now);
      d.setDate(d.getDate() - 6);
      dateStart = formatLocalDate(d);
      break;
    }
    case 'week': {
      const d = new Date(now);
      d.setDate(d.getDate() - 6);
      dateStart = formatLocalDate(d);
      break;
    }
    case 'month': {
      const d = new Date(now.getFullYear(), now.getMonth(), 1);
      dateStart = formatLocalDate(d);
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

function rangeLabel(key: string): string {
  switch (key) {
    case 'today': return '今日';
    case 'week': return '本周';
    case 'recent7days': return '近7天';
    case 'month': return '本月';
    default: return '本周';
  }
}

export const HomePage: React.FC = () => {
  const currentIdentity = useAppStore((s) => s.currentIdentity);
  const config = useAppStore((s) => s.config);
  const bills = useAppStore((s) => s.bills);
  const dailyTrend = useAppStore((s) => s.dailyTrend);
  const categoryDistribution = useAppStore((s) => s.categoryDistribution);
  const isLoadingStatistics = useAppStore((s) => s.isLoadingStatistics);
  const todaySummary = useAppStore((s) => s.todaySummary);
  const monthSummary = useAppStore((s) => s.monthSummary);
  const loadTodaySummary = useAppStore((s) => s.loadTodaySummary);
  const loadMonthSummary = useAppStore((s) => s.loadMonthSummary);
  const loadDailyTrend = useAppStore((s) => s.loadDailyTrend);
  const loadCategoryDistribution = useAppStore((s) => s.loadCategoryDistribution);
  const loadForgotCardStats = useAppStore((s) => s.loadForgotCardStats);
  const forgotCardStats = useAppStore((s) => s.forgotCardStats);
  const refreshStatistics = useAppStore((s) => s.refreshStatistics);
  const setShowStatisticsDialog = useAppStore((s) => s.setShowStatisticsDialog);

  const trendRange = config?.ui.home_trend_range ?? 'week';
  const categoryRange = config?.ui.home_category_range ?? 'month';

  const [isRefreshing, setIsRefreshing] = useState(false);
  const [homeTab, setHomeTab] = useState<string>('overview');

  useEffect(() => {
    if (!currentIdentity) return;
    const todayParams = getDateRangeParams('today', currentIdentity.id);
    const monthParams = getDateRangeParams('month', currentIdentity.id);
    const trendParams = getDateRangeParams(trendRange, currentIdentity.id);
    const categoryParams = getDateRangeParams(categoryRange, currentIdentity.id);
    loadTodaySummary(todayParams);
    loadMonthSummary(monthParams);
    loadDailyTrend(trendParams);
    loadCategoryDistribution(categoryParams);
    loadForgotCardStats(monthParams);
  }, [currentIdentity, trendRange, categoryRange]);

  const handleRefresh = async () => {
    setIsRefreshing(true);
    try {
      await refreshStatistics();
    } finally {
      setIsRefreshing(false);
    }
  };

  const recentBills = bills.slice(0, 5);
  const summaryCards = [
    {
      title: '今日消费',
      value: todaySummary ? `¥ ${Math.abs(todaySummary.total_expense).toFixed(2)}` : '加载中...',
      icon: <SubtractCircle24Regular />,
      color: 'var(--colorPaletteRedForeground3)',
    },
    {
      title: '本月消费',
      value: monthSummary ? `¥ ${Math.abs(monthSummary.total_expense).toFixed(2)}` : '加载中...',
      icon: <SubtractCircle24Regular />,
      color: 'var(--colorPaletteRedForeground3)',
    },
    {
      title: '本月充值',
      value: monthSummary ? `¥ ${monthSummary.total_income.toFixed(2)}` : '加载中...',
      icon: <AddCircle24Regular />,
      color: 'var(--colorPaletteGreenForeground3)',
    },
    {
      title: '卡片余额',
      value: '暂不可用',
      icon: <Money24Regular />,
      color: 'var(--colorBrandForeground1)',
    },
  ];

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
      {/* Header with Refresh Button */}
      <SectionEnterMotion>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 20 }}>
          <Title3>首页统计</Title3>
          <div style={{ display: 'flex', gap: 8 }}>
            <Button
              icon={<ArrowExpand24Regular />}
              appearance="secondary"
              size="small"
              onClick={() => setShowStatisticsDialog(true)}
              disabled={!currentIdentity}
            >
              查看更多
            </Button>
            <Button
              icon={
                <span className={isRefreshing ? 'motion-spin-indefinite' : undefined}>
                  <ArrowSync24Regular />
                </span>
              }
              appearance="secondary"
              size="small"
              onClick={handleRefresh}
              disabled={isRefreshing || !currentIdentity}
            >
              {isRefreshing ? '刷新中...' : '刷新统计'}
            </Button>
          </div>
        </div>
      </SectionEnterMotion>

      {/* Summary Cards */}
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))', gap: 12, marginBottom: 20 }}>
            {summaryCards.map((card, index) => (
              <CardEnterMotion key={card.title} delay={getStaggerDelay(index, 70, 90)}>
                <div>
                  <StatCard
                    title={card.title}
                    value={card.value}
                    icon={card.icon}
                    color={card.color}
                  />
                </div>
              </CardEnterMotion>
            ))}
          </div>

          {/* Charts Row */}
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12, marginBottom: 20 }}>
            <CardEnterMotion delay={getStaggerDelay(0, 90, 220)}>
              <Card className="motion-hover-lift motion-sheen" style={{ padding: 16 }}>
                <CardHeader>
                  <InfoLabel info="点击图例可切换显示/隐藏线条。">
                    {rangeLabel(trendRange)}消费趋势
                  </InfoLabel>
                </CardHeader>
                {isLoadingStatistics ? (
                  <div style={{ display: 'flex', justifyContent: 'center', padding: 40 }}>
                    <Spinner label="加载中..." />
                  </div>
                ) : (
                  <ExpenseTrendChart data={dailyTrend} />
                )}
              </Card>
            </CardEnterMotion>
            <CardEnterMotion delay={getStaggerDelay(1, 90, 220)}>
              <Card className="motion-hover-lift motion-sheen" style={{ padding: 16 }}>
                <CardHeader>
                  <InfoLabel info="点击扇区查看该分类详情。">
                    {rangeLabel(categoryRange)}消费分类占比
                  </InfoLabel>
                </CardHeader>
                <CategoryPieChart data={categoryDistribution} />
              </Card>
            </CardEnterMotion>
          </div>

          {/* Recent Transactions */}
          <CardEnterMotion delay={320}>
            <Card className="motion-hover-lift" style={{ padding: 16 }}>
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
                      className="motion-list-row"
                      style={{
                        display: 'flex',
                        justifyContent: 'space-between',
                        alignItems: 'center',
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
                        style={{ color: bill.item_type?.includes('充值') || bill.item_type?.includes('冲正') || bill.item_type?.includes('退款') ? 'var(--colorPaletteGreenForeground3)' : 'var(--colorPaletteRedForeground3)' }}
                      >
                        {formatBillMoney(bill.money, bill.item_type || '')}
                      </Text>
                    </div>
                  ))}
                </div>
              )}
            </Card>
          </CardEnterMotion>
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
  <Card className="motion-hover-lift motion-sheen" style={{ padding: 16 }}>
    <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 8 }}>
      <span className="motion-float" style={{ color }}>{icon}</span>
      <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>{title}</Text>
    </div>
    <Title3 block>{value}</Title3>
  </Card>
);
