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
  Text,
  Subtitle2,
  Card,
  CardHeader,
  Spinner,
  Title3,
  TabList,
  Tab,
  InfoLabel,
} from '@fluentui/react-components';
import { ChartMultiple24Regular } from '@fluentui/react-icons';
import { useAppStore } from '../../stores/appStore';
import { ExpenseTrendChart } from '../../components/Charts/ExpenseTrendChart';
import { CategoryPieChart } from '../../components/Charts/CategoryPieChart';
import { CategoryBarChart } from '../../components/Charts/CategoryBarChart';
import { PositionPieChart } from '../../components/Charts/PositionPieChart';
import { MealDistChart } from '../../components/Charts/MealDistChart';
import { ConsumptionDistributionChart } from '../../components/Charts/ConsumptionDistributionChart';
import { MerchantRankingChart } from '../../components/Charts/MerchantRankingChart';
import { MonthComparisonCard } from '../../components/Charts/MonthComparisonCard';
import * as tauri from '../../services/tauri';
import { formatLocalDate } from '../../utils/date';
import { getCategoryDisplayName, getAllCategories, getCategoryColor } from '../../utils/translation';
import {
  CardEnterMotion,
  PageEnterMotion,
  SectionEnterMotion,
  getStaggerDelay,
} from '../../components/Common/motion';

function buildParams(identityId: number, rangeKey: string): tauri.StatisticsParams {
  const now = new Date();
  let dateStart: string | undefined;
  let dateEnd = formatLocalDate(now);

  switch (rangeKey) {
    case 'today': {
      dateStart = dateEnd;
      break;
    }
    case 'recent7days': {
      const d = new Date(now);
      d.setDate(d.getDate() - 6);
      dateStart = formatLocalDate(d);
      break;
    }
    case 'week': {
      const day = now.getDay();
      const diff = day === 0 ? 6 : day - 1;
      const d = new Date(now);
      d.setDate(d.getDate() - diff);
      dateStart = formatLocalDate(d);
      break;
    }
    case 'month': {
      const d = new Date(now.getFullYear(), now.getMonth(), 1);
      dateStart = formatLocalDate(d);
      break;
    }
    case 'recent30days': {
      const d = new Date(now);
      d.setDate(d.getDate() - 29);
      dateStart = formatLocalDate(d);
      break;
    }
    case 'quarter': {
      const d = new Date(now);
      d.setMonth(d.getMonth() - 3);
      d.setDate(d.getDate() + 1);
      dateStart = formatLocalDate(d);
      break;
    }
    case 'halfYear': {
      const d = new Date(now);
      d.setMonth(d.getMonth() - 6);
      d.setDate(d.getDate() + 1);
      dateStart = formatLocalDate(d);
      break;
    }
    case 'year': {
      const d = new Date(now);
      d.setFullYear(d.getFullYear() - 1);
      d.setDate(d.getDate() + 1);
      dateStart = formatLocalDate(d);
      break;
    }
    case '3years': {
      const d = new Date(now);
      d.setFullYear(d.getFullYear() - 3);
      d.setDate(d.getDate() + 1);
      dateStart = formatLocalDate(d);
      break;
    }
    case '4years': {
      const d = new Date(now);
      d.setFullYear(d.getFullYear() - 4);
      d.setDate(d.getDate() + 1);
      dateStart = formatLocalDate(d);
      break;
    }
    case 'all': {
      dateStart = undefined;
      break;
    }
    case '30days': {
      const d = new Date(now);
      d.setDate(d.getDate() - 29);
      dateStart = formatLocalDate(d);
      break;
    }
    default: {
      dateStart = dateEnd;
      break;
    }
  }

  return { identityId, dateStart, dateEnd };
}

export const StatisticsDialog: React.FC = () => {
  const showStatisticsDialog = useAppStore((s) => s.showStatisticsDialog);
  const setShowStatisticsDialog = useAppStore((s) => s.setShowStatisticsDialog);
  const identities = useAppStore((s) => s.identities);
  const currentIdentity = useAppStore((s) => s.currentIdentity);
  const statisticsSummary = useAppStore((s) => s.statisticsSummary);
  const dailyTrend = useAppStore((s) => s.dailyTrend);
  const categoryDistribution = useAppStore((s) => s.categoryDistribution);
  const mealDistribution = useAppStore((s) => s.mealDistribution);
  const consumptionDistribution = useAppStore((s) => s.consumptionDistribution);
  const merchantRanking = useAppStore((s) => s.merchantRanking);
  const forgotCardStats = useAppStore((s) => s.forgotCardStats);
  const loadForgotCardStats = useAppStore((s) => s.loadForgotCardStats);
  const isLoadingStatistics = useAppStore((s) => s.isLoadingStatistics);
  const loadStatisticsSummary = useAppStore((s) => s.loadStatisticsSummary);
  const loadDailyTrend = useAppStore((s) => s.loadDailyTrend);
  const loadCategoryDistribution = useAppStore((s) => s.loadCategoryDistribution);
  const loadMealDistribution = useAppStore((s) => s.loadMealDistribution);
  const loadConsumptionDistribution = useAppStore((s) => s.loadConsumptionDistribution);
  const loadMerchantRanking = useAppStore((s) => s.loadMerchantRanking);

  const [selectedIdentityId, setSelectedIdentityId] = useState<string>(
    currentIdentity?.id?.toString() ?? ''
  );
  const [dateRange, setDateRange] = useState('month');
  const [chartTab, setChartTab] = useState<string>('overview');
  const [selectedCategory, setSelectedCategory] = useState<string>('all');
  const [categorySummaries, setCategorySummaries] = useState<Record<string, tauri.CategorySummary>>({});
  const [loadingCategories, setLoadingCategories] = useState(false);

  const loadAll = useCallback(
    (id: string, range: string) => {
      const identityId = parseInt(id);
      if (!id || isNaN(identityId)) return;
      const params = buildParams(identityId, range);
      loadStatisticsSummary(params);
      loadDailyTrend(params);
      loadCategoryDistribution(params);
      loadMealDistribution(params);
      loadConsumptionDistribution(params);
      loadMerchantRanking(params);
    loadForgotCardStats(params);
    },
    [loadStatisticsSummary, loadDailyTrend, loadCategoryDistribution, loadMealDistribution, loadConsumptionDistribution, loadMerchantRanking]
  );

  // Sync default identity
  useEffect(() => {
    if (!selectedIdentityId && currentIdentity) {
      setSelectedIdentityId(currentIdentity.id.toString());
    }
  }, [currentIdentity]);

  // Load data when filter changes
  useEffect(() => {
    loadAll(selectedIdentityId, dateRange);
  }, [selectedIdentityId, dateRange, loadAll]);

  // Load category summaries for quick overview
  useEffect(() => {
    const id = parseInt(selectedIdentityId);
    if (!selectedIdentityId || isNaN(id)) return;
    const params = buildParams(id, dateRange);
    setLoadingCategories(true);
    const categories = getAllCategories();
    Promise.all(
      categories.map((cat) =>
        tauri.get_category_summary({
          identityId: id,
          category: getCategoryDisplayName(cat),
          dateStart: params.dateStart,
          dateEnd: params.dateEnd,
        }).catch(() => null)
      )
    ).then((results) => {
      const map: Record<string, tauri.CategorySummary> = {};
      results.forEach((r, i) => {
        if (r) map[categories[i]] = r;
      });
      setCategorySummaries(map);
      setLoadingCategories(false);
    });
  }, [selectedIdentityId, dateRange]);

  // Handle category click from pie chart
  const handleCategoryClick = useCallback((categoryName: string) => {
    setSelectedCategory(categoryName);
    setChartTab('category');
  }, []);

  const summary = statisticsSummary;

  // Filter category data by selected type
  const filteredCategoryData = selectedCategory === 'all'
    ? categoryDistribution
    : categoryDistribution.filter((c) => c.name === selectedCategory);

  const handleOpenChange = useCallback(
    (_: unknown, data: { open: boolean }) => {
      if (!data.open) {
        setChartTab('overview');
        setSelectedCategory('all');
        setShowStatisticsDialog(false);
      }
    },
    [setShowStatisticsDialog]
  );

  return (
    <Dialog open={showStatisticsDialog} onOpenChange={handleOpenChange}>
      <DialogSurface style={{ maxWidth: 800, width: '90vw' }}>
        <DialogBody>
          <DialogTitle>
            <ChartMultiple24Regular style={{ marginRight: 8 }} />
            统计分析
          </DialogTitle>
          <DialogContent>
            {/* Filters */}
            <SectionEnterMotion>
              <div style={{ display: 'flex', gap: 8, marginBottom: 12, flexWrap: 'wrap' }}>
                <Dropdown
                  value={
                    dateRange === 'today' ? '今天' :
                    dateRange === 'recent7days' ? '最近7天' :
                    dateRange === 'week' ? '本周' :
                    dateRange === 'month' ? '本月' :
                    dateRange === 'recent30days' ? '最近30天' :
                    dateRange === 'quarter' ? '最近3个月' :
                    dateRange === 'halfYear' ? '最近半年' :
                    dateRange === 'year' ? '最近一年' :
                    dateRange === '30days' ? '近30天' : '本月'
                  }
                  selectedOptions={[dateRange]}
                  onOptionSelect={(_, data) => setDateRange(data.optionValue ?? 'month')}
                  style={{ minWidth: 120 }}
                >
                  <Option value="today">今天</Option>
                  <Option value="recent7days">最近7天</Option>
                  <Option value="week">本周</Option>
                  <Option value="month">本月</Option>
                  <Option value="recent30days">最近30天</Option>
                  <Option value="quarter">最近3个月</Option>
                  <Option value="halfYear">最近半年</Option>
                  <Option value="year">最近一年</Option>
                  <Option value="3years">最近3年</Option>
                  <Option value="4years">最近4年</Option>
                  <Option value="all">全部时间</Option>
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
                <Dropdown
                  value={selectedCategory === 'all' ? '全部分类' : getCategoryDisplayName(selectedCategory)}
                  selectedOptions={[selectedCategory]}
                  onOptionSelect={(_, data) => setSelectedCategory(data.optionValue ?? 'all')}
                  style={{ minWidth: 130 }}
                >
                  <Option value="all">全部分类</Option>
                  {getAllCategories().map((cat) => (
                    <Option key={cat} value={cat}>
                      {getCategoryDisplayName(cat)}
                    </Option>
                  ))}
                </Dropdown>
              </div>
            </SectionEnterMotion>

            {/* Chart tab switcher */}
            <SectionEnterMotion delay={50}>
              <div>
                <TabList selectedValue={chartTab} onTabSelect={(_, data) => setChartTab(data.value as string)} style={{ marginBottom: 12 }}>
                  <Tab value="overview">总览</Tab>
                  <Tab value="category">分类分析</Tab>
                  <Tab value="position">位置分布</Tab>
                  <Tab value="compare">月度对比</Tab>
                  <Tab value="forgot">忘记拔卡</Tab>
                </TabList>
              </div>
            </SectionEnterMotion>

            <PageEnterMotion key={chartTab}>
              <div>
                {chartTab === 'overview' && (
                  <>
                {/* Summary */}
                <Card className="motion-hover-lift motion-sheen" style={{ padding: 16, marginBottom: 12 }}>
                  <CardHeader>
                    <Subtitle2>统计摘要</Subtitle2>
                  </CardHeader>
                  {isLoadingStatistics ? (
                    <div style={{ display: 'flex', justifyContent: 'center', padding: 24 }}>
                      <Spinner label="加载中..." />
                    </div>
                  ) : summary ? (
                    <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: 16, marginTop: 8 }}>
                      <SummaryItem label="总消费" value={`¥${summary.total_expense.toFixed(2)}`} color="var(--colorPaletteRedForeground3)" delay={getStaggerDelay(0, 40)} />
                      <SummaryItem label="总充值" value={`¥${summary.total_income.toFixed(2)}`} color="var(--colorPaletteGreenForeground3)" delay={getStaggerDelay(1, 40)} />
                      <SummaryItem label="净支出" value={`¥${summary.net_expense.toFixed(2)}`} color="var(--colorBrandForeground1)" delay={getStaggerDelay(2, 40)} />
                      <SummaryItem label="日均消费" value={`¥${summary.daily_average.toFixed(2)}`} delay={getStaggerDelay(3, 40)} />
                      <SummaryItem label="消费笔数" value={`${summary.expense_count}笔`} delay={getStaggerDelay(4, 40)} />
                      <SummaryItem label="充值笔数" value={`${summary.income_count}笔`} delay={getStaggerDelay(5, 40)} />
                    </div>
                  ) : (
                    <Text style={{ color: 'var(--colorNeutralForeground3)', textAlign: 'center', padding: 16, display: 'block' }}>
                      暂无数据
                    </Text>
                  )}
                </Card>

                {/* Category quick summary */}
                <Card className="motion-hover-lift" style={{ padding: 16, marginBottom: 12 }}>
                  <CardHeader>
                    <InfoLabel info="按消费类型展示各分类的总金额和笔数。数据实时计算。">
                      分类消费概览
                    </InfoLabel>
                  </CardHeader>
                  {loadingCategories ? (
                    <div style={{ display: 'flex', justifyContent: 'center', padding: 16 }}>
                      <Spinner size="small" label="加载中..." />
                    </div>
                  ) : (
                    <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(150px, 1fr))', gap: 8, marginTop: 8 }}>
                      {getAllCategories().map((cat) => {
                        const summary = categorySummaries[cat];
                        return (
                          <div
                            key={cat}
                            onClick={() => { setSelectedCategory(cat); setChartTab('category'); }}
                            className="motion-hover-lift"
                            style={{
                              padding: '10px 12px',
                              borderRadius: 8,
                              border: `1px solid ${getCategoryColor(cat)}`,
                              cursor: 'pointer',
                              background: selectedCategory === cat ? `${getCategoryColor(cat)}18` : 'transparent',
                            }}
                          >
                            <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 4 }}>
                              <span style={{
                                width: 8, height: 8, borderRadius: '50%',
                                background: getCategoryColor(cat), display: 'inline-block',
                              }} />
                              <Text size={200} weight="semibold">{getCategoryDisplayName(cat)}</Text>
                            </div>
                            {summary ? (
                              <>
                                <Text size={300} weight="bold">¥{summary.total_amount.toFixed(2)}</Text>
                                <Text size={100} style={{ color: 'var(--colorNeutralForeground3)', display: 'block' }}>
                                  {summary.count}笔 · 日均¥{summary.daily_average.toFixed(2)}
                                </Text>
                              </>
                            ) : (
                              <Text size={100} style={{ color: 'var(--colorNeutralForeground3)' }}>暂无数据</Text>
                            )}
                          </div>
                        );
                      })}
                    </div>
                  )}
                </Card>

                {/* Trend chart */}
                <Card className="motion-hover-lift motion-sheen" style={{ padding: 16, marginBottom: 12 }}>
                  <CardHeader>
                    <InfoLabel info="点击图例可切换显示/隐藏线条。点击数据点可查看当日详情。">
                      消费趋势
                    </InfoLabel>
                  </CardHeader>
                  <ExpenseTrendChart data={dailyTrend} />
                </Card>

                {/* Two column charts */}
                <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12, marginBottom: 12 }}>
                  <Card className="motion-hover-lift motion-sheen" style={{ padding: 16 }}>
                    <CardHeader>
                      <InfoLabel info="点击扇区可查看该分类详情。">
                        消费分类占比
                      </InfoLabel>
                    </CardHeader>
                    <CategoryPieChart data={categoryDistribution} onCategoryClick={handleCategoryClick} />
                  </Card>
                  <Card className="motion-hover-lift motion-sheen" style={{ padding: 16 }}>
                    <CardHeader>
                      <Subtitle2>用餐时段分布</Subtitle2>
                    </CardHeader>
                    <MealDistChart data={mealDistribution} />
                  </Card>
                </div>

                <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12, marginBottom: 12 }}>
                  <Card className="motion-hover-lift motion-sheen" style={{ padding: 16 }}>
                    <CardHeader>
                      <Subtitle2>消费金额分布</Subtitle2>
                    </CardHeader>
                    <ConsumptionDistributionChart data={consumptionDistribution} />
                  </Card>
                  <Card className="motion-hover-lift motion-sheen" style={{ padding: 16 }}>
                    <CardHeader>
                      <Subtitle2>商户消费排行</Subtitle2>
                    </CardHeader>
                    <MerchantRankingChart data={merchantRanking} />
                  </Card>
                </div>

                {/* Forgot Card Stats */}
                {forgotCardStats && forgotCardStats.count > 0 && (
                <Card className="motion-hover-lift" style={{ padding: 16, marginBottom: 12, borderLeft: '4px solid var(--colorPaletteRedForeground3)' }}>
                  <CardHeader>
                    <InfoLabel info={
                      forgotCardStats.items.length > 0
                        ? ['洗澡消费5元的记录（可能忘记拔卡）：', ...forgotCardStats.items.slice(0, 20).map((i) => `${i.date} ${i.time} ¥${i.amount} ${i.targetUser}`), forgotCardStats.items.length > 20 ? '...' : ''].join('\n')
                        : '洗澡上限为5元，消费5元意味着可能忘记拔卡'
                    }>
                      <Subtitle2 style={{ color: 'var(--colorPaletteRedForeground3)' }}>忘记拔卡提醒</Subtitle2>
                    </InfoLabel>
                  </CardHeader>
                  <div style={{ display: 'flex', gap: 24, alignItems: 'center' }}>
                    <div>
                      <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>洗澡消费达上限次数</Text>
                      <Title3 block style={{ color: 'var(--colorPaletteRedForeground3)' }}>{forgotCardStats.count} 次</Title3>
                    </div>
                    <div>
                      <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>累计金额</Text>
                      <Title3 block style={{ color: 'var(--colorPaletteRedForeground3)' }}>¥ {forgotCardStats.totalAmount.toFixed(2)}</Title3>
                    </div>
                  </div>
                </Card>
                )}
                  </>
                )}

                {chartTab === 'category' && (
                  <>
                {/* Category bar chart */}
                <Card className="motion-hover-lift motion-sheen" style={{ padding: 16, marginBottom: 12 }}>
                  <CardHeader>
                    <InfoLabel info="按消费类型（食堂、淋浴、电费等）展示金额分布。">
                      分类金额排行
                    </InfoLabel>
                  </CardHeader>
                  <CategoryBarChart data={filteredCategoryData} />
                </Card>

                {/* Category pie chart */}
                <Card className="motion-hover-lift motion-sheen" style={{ padding: 16, marginBottom: 12 }}>
                  <CardHeader>
                    <Subtitle2>分类占比详情</Subtitle2>
                  </CardHeader>
                  <div style={{ display: 'flex', justifyContent: 'center' }}>
                    <div style={{ width: '70%' }}>
                      <CategoryPieChart data={filteredCategoryData} onCategoryClick={handleCategoryClick} />
                    </div>
                  </div>
                </Card>

                {/* Category legend/tags */}
                <Card className="motion-hover-lift" style={{ padding: 16 }}>
                  <CardHeader>
                    <Subtitle2>分类图例</Subtitle2>
                  </CardHeader>
                  <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
                    {getAllCategories().map((cat) => (
                      <div
                        key={cat}
                        onClick={() => setSelectedCategory(selectedCategory === cat ? 'all' : cat)}
                        className="motion-hover-lift"
                        style={{
                          display: 'inline-flex',
                          alignItems: 'center',
                          gap: 6,
                          padding: '4px 12px',
                          borderRadius: 16,
                          cursor: 'pointer',
                          background: selectedCategory === cat ? getCategoryColor(cat) : 'transparent',
                          border: `1px solid ${getCategoryColor(cat)}`,
                          color: selectedCategory === cat ? '#fff' : getCategoryColor(cat),
                          fontSize: 13,
                        }}
                      >
                        <span style={{
                          width: 8,
                          height: 8,
                          borderRadius: '50%',
                          background: getCategoryColor(cat),
                          display: 'inline-block',
                        }} />
                        {getCategoryDisplayName(cat)}
                      </div>
                    ))}
                  </div>
                </Card>
                  </>
                )}

                {chartTab === 'position' && (
                  <>
                {/* Position pie chart */}
                <Card className="motion-hover-lift motion-sheen" style={{ padding: 16, marginBottom: 12 }}>
                  <CardHeader>
                    <InfoLabel info="根据商户名称映射到食堂楼栋位置，展示各位置的消费分布。悬停查看详情。">
                      消费位置分布
                    </InfoLabel>
                  </CardHeader>
                  <PositionPieChart data={merchantRanking} />
                </Card>

                {/* Merchant ranking */}
                <Card className="motion-hover-lift motion-sheen" style={{ padding: 16 }}>
                  <CardHeader>
                    <Subtitle2>商户排行详情</Subtitle2>
                  </CardHeader>
                  <MerchantRankingChart data={merchantRanking} />
                </Card>
                  </>
                )}

                {chartTab === 'compare' && (
                  <Card className="motion-hover-lift motion-sheen" style={{ padding: 16 }}>
                    <CardHeader>
                      <InfoLabel info="对比本月与上月的消费变化情况。">
                        月度消费对比
                      </InfoLabel>
                    </CardHeader>
                    <MonthComparisonCard identityId={parseInt(selectedIdentityId)} />
                  </Card>
                )}

                {chartTab === 'forgot' && (
                  <Card className="motion-hover-lift" style={{ padding: 16 }}>
                    <CardHeader>
                      <InfoLabel info="洗澡上限为5元，消费恰好5元意味着水龙头一直开着，可能忘记拔卡。">
                        忘记拔卡记录
                      </InfoLabel>
                    </CardHeader>
                    {forgotCardStats ? (
                      forgotCardStats.count > 0 ? (
                        <div>
                          <div style={{ display: 'flex', gap: 16, marginBottom: 16 }}>
                            <Text weight="semibold" style={{ color: 'var(--colorPaletteRedForeground3)' }}>
                              共 {forgotCardStats.count} 次，累计 ¥{forgotCardStats.totalAmount.toFixed(2)}
                            </Text>
                          </div>
                          <div style={{ display: 'grid', gap: 8 }}>
                            {forgotCardStats.items.map((item) => (
                              <div
                                key={item.id}
                                style={{
                                  display: 'flex',
                                  justifyContent: 'space-between',
                                  alignItems: 'center',
                                  padding: '8px 12px',
                                  borderRadius: 6,
                                  border: '1px solid var(--colorNeutralStroke2)',
                                  background: 'var(--colorNeutralBackground2)',
                                }}
                              >
                                <div>
                                  <Text size={200} weight="semibold">{item.date}</Text>
                                  <Text size={200} style={{ color: 'var(--colorNeutralForeground3)', marginLeft: 8 }}>{item.time}</Text>
                                </div>
                                <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
                                  <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>{item.targetUser}</Text>
                                  <Text size={200} weight="semibold" style={{ color: 'var(--colorPaletteRedForeground3)' }}>¥{item.amount.toFixed(2)}</Text>
                                </div>
                              </div>
                            ))}
                          </div>
                        </div>
                      ) : (
                        <Text size={200} style={{ color: 'var(--colorNeutralForeground3)', padding: 24, display: 'block', textAlign: 'center' }}>
                          当前时间范围内没有发现忘记拔卡的记录
                        </Text>
                      )
                    ) : (
                      <div style={{ display: 'flex', justifyContent: 'center', padding: 24 }}>
                        <Spinner label="加载中..." />
                      </div>
                    )}
                  </Card>
                )}
              </div>
            </PageEnterMotion>
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
  delay?: number;
}> = ({ label, value, color, delay = 0 }) => (
  <CardEnterMotion delay={delay}>
    <div>
      <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }} block>
        {label}
      </Text>
      <Title3 style={{ color: color ?? 'inherit' }}>{value}</Title3>
    </div>
  </CardEnterMotion>
);
