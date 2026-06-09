import React, { useEffect, useState, useCallback } from 'react';
import {
  Card,
  CardHeader,
  Text,
  Title3,
  Subtitle2,
  Button,
  InfoLabel,
  Badge,
  Dialog,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  DialogActions,
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
import type { BillItem } from '../../types';
import { formatLocalDate } from '../../utils/date';
import { BillDetailDialog } from '../../components/Common/BillDetailDialog';
import { ContextMenu } from '../../components/Common/ContextMenu';
import {
  SkeletonChartBlock,
  SkeletonLine,
} from '../../components/Common/LoadingSkeleton';
import {
  CardEnterMotion,
  SectionEnterMotion,
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
  const accounts = useAppStore((s) => s.accounts);
  const config = useAppStore((s) => s.config);
  const bills = useAppStore((s) => s.bills);
  const isLoadingBills = useAppStore((s) => s.isLoading);
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
  const [detailBill, setDetailBill] = useState<BillItem | null>(null);
  const [cardBalance, setCardBalance] = useState<string | null>(null);
  const [cardBalanceLoaded, setCardBalanceLoaded] = useState(false);
  const [cardBalanceLoading, setCardBalanceLoading] = useState(false);
  const [showRefreshBalanceDialog, setShowRefreshBalanceDialog] = useState(false);
  const [balanceDialogAccountId, setBalanceDialogAccountId] = useState<number | null>(null);

  // 当前身份下首选可用账号
  const currentAccount = accounts.find((a) => a.enable) ?? accounts[0] ?? null;

  const loadCardBalance = useCallback(async () => {
    if (!currentAccount) {
      setCardBalance(null);
      setCardBalanceLoaded(true);
      return;
    }
    setCardBalanceLoading(true);
    try {
      const cached = await tauri.get_cached_person_account(currentAccount.id);
      // 优先用 raw 字符串, 但即便 raw 为空, 只要 cash_balance > 0 也显示 (避免老缓存一直是 "点击刷新")
      if (cached && (cached.cash_balance_raw || (cached.cash_balance ?? 0) > 0)) {
        const raw = cached.cash_balance_raw?.trim();
        const display = raw && raw.length > 0 ? raw : cached.cash_balance.toFixed(2);
        setCardBalance(`${display} 元`);
      } else {
        setCardBalance(null);
      }
    } catch {
      setCardBalance(null);
    } finally {
      setCardBalanceLoaded(true);
      setCardBalanceLoading(false);
    }
  }, [currentAccount]);

  useEffect(() => {
    setCardBalance(null);
    setCardBalanceLoaded(false);
    void loadCardBalance();
  }, [loadCardBalance]);

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
      await loadCardBalance();
    } finally {
      setIsRefreshing(false);
    }
  };

  const handleFetchBalance = useCallback(async () => {
    if (!currentAccount) return;
    await fetchBalanceFor(currentAccount.id);
  }, [currentAccount]);

  const fetchBalanceFor = useCallback(async (accountId: number) => {
    setCardBalanceLoading(true);
    try {
      const info = await tauri.fetch_person_account(accountId);
      setCardBalance(info.cash_balance_raw ? `${info.cash_balance_raw} 元` : '0.00 元');
    } catch {
      setCardBalance(null);
    } finally {
      setCardBalanceLoading(false);
    }
  }, [currentAccount]);

  const handleOpenBillDetail = useCallback(async (billId: number) => {
    if (!currentIdentity) return;
    try {
      const bill = await tauri.get_bill_detail(currentIdentity.id, billId);
      setDetailBill(bill);
    } catch (e) {
      console.error('Failed to load home bill detail:', e);
    }
  }, [currentIdentity]);

  const recentBills = bills.slice(0, 5);
  const hasForgotCardRisk = (forgotCardStats?.count ?? 0) > 0;
  const summaryCards = [
    {
      title: '今日消费',
      value: todaySummary ? `¥ ${Math.abs(todaySummary.total_expense).toFixed(2)}` : '加载中...',
      icon: <SubtractCircle24Regular />,
      color: 'var(--colorPaletteRedForeground3)',
      tone: 'expense' as const,
    },
    {
      title: '本月消费',
      value: monthSummary ? `¥ ${Math.abs(monthSummary.total_expense).toFixed(2)}` : '加载中...',
      icon: <SubtractCircle24Regular />,
      color: 'var(--colorPaletteRedForeground3)',
      tone: 'expense' as const,
    },
    {
      title: '本月充值',
      value: monthSummary ? `¥ ${monthSummary.total_income.toFixed(2)}` : '加载中...',
      icon: <AddCircle24Regular />,
      color: 'var(--colorPaletteGreenForeground3)',
      tone: 'income' as const,
    },
    {
      title: '卡片余额',
      value: cardBalanceLoading
        ? '加载中...'
        : cardBalance
          ? `¥ ${cardBalance.replace(' 元', '')}`
          : cardBalanceLoaded
            ? '点击刷新'
            : '加载中...',
      icon: <Money24Regular />,
      color: 'var(--colorBrandForeground1)',
      tone: 'brand' as const,
      onClick: currentAccount
        ? () => {
            setBalanceDialogAccountId(currentAccount.id);
            setShowRefreshBalanceDialog(true);
          }
        : undefined,
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
    <div className="home-page">
      <SectionEnterMotion>
        <div className="home-page__header">
          <div className="home-page__title-block">
            <Title3>首页统计</Title3>
            <Text size={200} className="home-page__subtitle">
              快速查看近期消费、分类趋势和异常提醒
            </Text>
          </div>
          <div className="home-page__actions">
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

      <div className="home-page__summary-grid">
        {summaryCards.map((card, index) => (
          <CardEnterMotion key={card.title} delay={getStaggerDelay(index, 70, 90)}>
            <div onClick={card.onClick} style={card.onClick ? { cursor: 'pointer' } : undefined}>
              <StatCard
                title={card.title}
                value={card.value}
                icon={card.icon}
                color={card.color}
                tone={card.tone}
              />
            </div>
          </CardEnterMotion>
        ))}
      </div>

      <div className="home-page__content-grid">
        <div className="home-page__main-column">
          <div className="home-page__chart-grid">
            <CardEnterMotion delay={getStaggerDelay(0, 90, 220)}>
              <Card className="home-page__panel home-page__panel--trend" style={{ padding: 16 }}>
                <CardHeader>
                  <InfoLabel info="点击图例可切换显示/隐藏线条。">
                    {rangeLabel(trendRange)}消费趋势
                  </InfoLabel>
                </CardHeader>
                {isLoadingStatistics ? (
                  <SkeletonChartBlock height={240} />
                ) : (
                  <ExpenseTrendChart data={dailyTrend} />
                )}
              </Card>
            </CardEnterMotion>
            <CardEnterMotion delay={getStaggerDelay(1, 90, 220)}>
              <Card className="home-page__panel home-page__panel--category" style={{ padding: 16 }}>
                <CardHeader>
                  <InfoLabel info="点击扇区查看该分类详情。">
                    {rangeLabel(categoryRange)}消费分类占比
                  </InfoLabel>
                </CardHeader>
                {isLoadingStatistics ? <SkeletonChartBlock height={220} /> : <CategoryPieChart data={categoryDistribution} />}
              </Card>
            </CardEnterMotion>
          </div>

          <CardEnterMotion delay={320}>
            <Card className="home-page__panel home-page__panel--recent" style={{ padding: 16 }}>
              <CardHeader>
                <Subtitle2>最近交易</Subtitle2>
              </CardHeader>
              {isLoadingBills ? (
                <div style={{ display: 'grid', gap: 10 }}>
                  {Array.from({ length: 5 }).map((_, index) => (
                    <div
                      key={index}
                      style={{
                        display: 'flex',
                        justifyContent: 'space-between',
                        alignItems: 'center',
                        padding: '10px 0',
                        borderBottom: '1px solid var(--colorNeutralStroke2)',
                      }}
                    >
                      <div style={{ display: 'grid', gap: 6, width: '100%' }}>
                        <SkeletonLine width="48%" height={12} />
                        <SkeletonLine width="32%" height={10} />
                      </div>
                      <SkeletonLine width={64} height={14} />
                    </div>
                  ))}
                </div>
              ) : recentBills.length === 0 ? (
                <Text size={200} style={{ color: 'var(--colorNeutralForeground3)', padding: 24, display: 'block', textAlign: 'center' }}>
                  暂无交易记录
                </Text>
              ) : (
                <div className="home-page__recent-list">
                  {recentBills.map((bill) => (
                    <ContextMenu
                      key={bill.id}
                      actions={[
                        {
                          key: 'detail',
                          label: '查看详情',
                          onSelect: () => void handleOpenBillDetail(bill.id),
                        },
                        {
                          key: 'copy-target',
                          label: '复制对方账户',
                          onSelect: () => navigator.clipboard.writeText(bill.target_user || ''),
                        },
                        {
                          key: 'copy-money',
                          label: '复制金额',
                          onSelect: () => navigator.clipboard.writeText(formatBillMoney(bill.money, bill.item_type || '')),
                        },
                      ]}
                    >
                      <div
                        className="motion-list-row"
                        data-app-context-menu-root="true"
                        onClick={() => { void handleOpenBillDetail(bill.id); }}
                        style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', cursor: 'pointer' }}
                      >
                        <div style={{ display: 'grid', gap: 4 }}>
                          <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                            <Text block size={200}>{bill.item_type}</Text>
                            {bill.is_combined && <Badge appearance="outline" size="small">合并</Badge>}
                          </div>
                          <Text size={100} style={{ color: 'var(--colorNeutralForeground3)' }}>
                            {bill.date_time_formatted || `${bill.date_str} ${bill.time_str_formatted}`}
                          </Text>
                          <Text size={100} style={{ color: 'var(--colorNeutralForeground3)' }}>
                            {bill.target_user || '—'}
                          </Text>
                        </div>
                        <Text
                          weight="semibold"
                          style={{ color: bill.item_type?.includes('充值') || bill.item_type?.includes('冲正') || bill.item_type?.includes('退款') ? 'var(--colorPaletteGreenForeground3)' : 'var(--colorPaletteRedForeground3)' }}
                        >
                          {formatBillMoney(bill.money, bill.item_type || '')}
                        </Text>
                      </div>
                    </ContextMenu>
                  ))}
                </div>
              )}
            </Card>
          </CardEnterMotion>
        </div>

        <div className="home-page__side-column">
          <CardEnterMotion delay={getStaggerDelay(2, 90, 220)}>
            <Card className="home-page__panel home-page__panel--compare" style={{ padding: 16 }}>
              <CardHeader>
                <Subtitle2>月度对比</Subtitle2>
              </CardHeader>
              <MonthComparisonCard identityId={currentIdentity.id} />
            </Card>
          </CardEnterMotion>

          <CardEnterMotion delay={getStaggerDelay(3, 90, 220)}>
            <Card
              className={hasForgotCardRisk ? 'home-page__panel home-page__panel--alert' : 'home-page__panel home-page__panel--safe'}
              style={{ padding: 16 }}
            >
              <CardHeader
                header={
                  <div className="home-page__aside-header">
                    <Subtitle2>异常提醒</Subtitle2>
                    <Badge
                      appearance="filled"
                      color={hasForgotCardRisk ? 'danger' : 'success'}
                      size="small"
                    >
                      {hasForgotCardRisk ? `${forgotCardStats?.count ?? 0} 条` : '正常'}
                    </Badge>
                  </div>
                }
              />
              <div className="home-page__aside-body">
                <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
                  疑似忘拔卡统计
                </Text>
                <Title3 block>
                  {forgotCardStats ? `${forgotCardStats.count} 次` : '加载中...'}
                </Title3>
                <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
                  {forgotCardStats
                    ? `累计金额 ¥${forgotCardStats.totalAmount.toFixed(2)}`
                    : '正在检查高风险记录'}
                </Text>
                <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
                  {hasForgotCardRisk
                    ? '建议到统计详情里继续核对洗浴消费记录。'
                    : '当前没有检测到明显的忘拔卡高风险记录。'}
                </Text>
              </div>
            </Card>
          </CardEnterMotion>
        </div>
      </div>

      {detailBill && (
        <BillDetailDialog bill={detailBill} onClose={() => setDetailBill(null)} />
      )}

      <Dialog
        open={showRefreshBalanceDialog}
        onOpenChange={(_, data) => setShowRefreshBalanceDialog(data.open)}
      >
        <DialogSurface>
          <DialogBody>
            <DialogTitle>刷新一卡通余额</DialogTitle>
            <DialogContent>
              即将从校园卡服务器拉取最新余额与个人信息, 可能需要输入验证码 (取决于配置)。
              确定继续吗?
            </DialogContent>
            <DialogActions>
              <Button
                appearance="subtle"
                onClick={() => {
                  setShowRefreshBalanceDialog(false);
                  setBalanceDialogAccountId(null);
                }}
              >
                取消
              </Button>
              <Button
                appearance="primary"
                disabled={cardBalanceLoading || !balanceDialogAccountId}
                onClick={() => {
                  const id = balanceDialogAccountId;
                  setShowRefreshBalanceDialog(false);
                  setBalanceDialogAccountId(null);
                  if (id) {
                    void fetchBalanceFor(id);
                  }
                }}
              >
                {cardBalanceLoading ? '刷新中...' : '确定刷新'}
              </Button>
            </DialogActions>
          </DialogBody>
        </DialogSurface>
      </Dialog>
    </div>
  );
};

interface StatCardProps {
  title: string;
  value: string;
  icon: React.ReactNode;
  color: string;
  tone: 'expense' | 'income' | 'brand';
}

const StatCard: React.FC<StatCardProps> = ({ title, value, icon, color, tone }) => (
  <Card className={`home-page__stat-card home-page__stat-card--${tone}`} style={{ padding: 16 }}>
    <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 8 }}>
      <span className="motion-float" style={{ color }}>{icon}</span>
      <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>{title}</Text>
    </div>
    <Title3 block>{value}</Title3>
  </Card>
);
