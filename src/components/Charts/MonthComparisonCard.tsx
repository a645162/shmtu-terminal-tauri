import React, { useEffect, useState } from 'react';
import { Text, Title3, Spinner } from '@fluentui/react-components';
import { useAppStore } from '../../stores/appStore';
import * as tauri from '../../services/tauri';

interface MonthComparisonData {
  currentMonth: string;
  lastMonth: string;
  currentExpense: number;
  lastExpense: number;
  changeAmount: number;
  changePercent: number;
  currentIncome: number;
  lastIncome: number;
  currentCount: number;
  lastCount: number;
}

interface Props {
  identityId?: number;
}

function getMonthDateRange(year: number, month: number) {
  const start = `${year}-${String(month).padStart(2, '0')}-01`;
  const lastDay = new Date(year, month, 0).getDate();
  const end = `${year}-${String(month).padStart(2, '0')}-${String(lastDay).padStart(2, '0')}`;
  return { start, end };
}

export const MonthComparisonCard: React.FC<Props> = ({ identityId }) => {
  const currentIdentity = useAppStore((s) => s.currentIdentity);
  const theme = useAppStore((s) => s.theme);
  const [data, setData] = useState<MonthComparisonData | null>(null);
  const [loading, setLoading] = useState(false);

  const id = identityId ?? currentIdentity?.id;

  useEffect(() => {
    if (!id) return;

    const now = new Date();
    const currentYear = now.getFullYear();
    const currentMonth = now.getMonth() + 1;

    const currentRange = getMonthDateRange(currentYear, currentMonth);

    let lastYear = currentYear;
    let lastMonth = currentMonth - 1;
    if (lastMonth === 0) {
      lastMonth = 12;
      lastYear = currentYear - 1;
    }
    const lastRange = getMonthDateRange(lastYear, lastMonth);

    setLoading(true);

    Promise.all([
      tauri.get_statistics_summary({ identityId: id, dateStart: currentRange.start, dateEnd: currentRange.end }),
      tauri.get_statistics_summary({ identityId: id, dateStart: lastRange.start, dateEnd: lastRange.end }),
    ])
      .then(([current, last]) => {
        const changeAmount = current.total_expense - last.total_expense;
        const changePercent = last.total_expense !== 0
          ? (changeAmount / Math.abs(last.total_expense)) * 100
          : 0;

        setData({
          currentMonth: `${currentYear}年${currentMonth}月`,
          lastMonth: `${lastYear}年${lastMonth}月`,
          currentExpense: current.total_expense,
          lastExpense: last.total_expense,
          changeAmount,
          changePercent,
          currentIncome: current.total_income,
          lastIncome: last.total_income,
          currentCount: current.expense_count,
          lastCount: last.expense_count,
        });
      })
      .catch((e) => console.error('Failed to load month comparison:', e))
      .finally(() => setLoading(false));
  }, [id]);

  if (!id) return null;

  const textColor = theme === 'dark' ? '#e0e0e0' : '#333';
  const mutedColor = 'var(--colorNeutralForeground3)';
  const positiveColor = 'var(--colorPaletteRedForeground3)';
  const negativeColor = 'var(--colorPaletteGreenForeground3)';

  if (loading) {
    return (
      <div style={{ display: 'flex', justifyContent: 'center', padding: 24 }}>
        <Spinner label="加载对比数据..." />
      </div>
    );
  }

  if (!data) {
    return (
      <Text style={{ color: mutedColor, textAlign: 'center', padding: 16, display: 'block' }}>
        暂无对比数据
      </Text>
    );
  }

  const isUp = data.changeAmount > 0;
  const changeColor = isUp ? positiveColor : negativeColor;
  const changeArrow = isUp ? '↑' : '↓';

  return (
    <div>
      <div style={{ display: 'grid', gridTemplateColumns: '1fr auto 1fr', gap: 16, alignItems: 'center', marginBottom: 16 }}>
        <div style={{ textAlign: 'center' }}>
          <Text size={200} style={{ color: mutedColor }} block>
            {data.lastMonth}
          </Text>
          <Title3 style={{ color: textColor }}>¥{Math.abs(data.lastExpense).toFixed(2)}</Title3>
          <Text size={200} style={{ color: mutedColor }} block>
            消费
          </Text>
        </div>

        <div style={{ textAlign: 'center', minWidth: 100 }}>
          <Text size={400} weight="bold" style={{ color: changeColor, fontSize: 20 }} block>
            {changeArrow} {Math.abs(data.changePercent).toFixed(1)}%
          </Text>
          <Text size={200} style={{ color: changeColor }} block>
            {isUp ? '增加' : '减少'} ¥{Math.abs(data.changeAmount).toFixed(2)}
          </Text>
        </div>

        <div style={{ textAlign: 'center' }}>
          <Text size={200} style={{ color: mutedColor }} block>
            {data.currentMonth}
          </Text>
          <Title3 style={{ color: textColor }}>¥{Math.abs(data.currentExpense).toFixed(2)}</Title3>
          <Text size={200} style={{ color: mutedColor }} block>
            消费
          </Text>
        </div>
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
        <div style={{ padding: 8, background: theme === 'dark' ? '#2d2d2d' : '#f5f5f5', borderRadius: 6 }}>
          <Text size={200} style={{ color: mutedColor }} block>本月充值</Text>
          <Text weight="semibold" style={{ color: negativeColor }}>¥{data.currentIncome.toFixed(2)}</Text>
          <Text size={100} style={{ color: mutedColor }} block>
            上月: ¥{data.lastIncome.toFixed(2)}
          </Text>
        </div>
        <div style={{ padding: 8, background: theme === 'dark' ? '#2d2d2d' : '#f5f5f5', borderRadius: 6 }}>
          <Text size={200} style={{ color: mutedColor }} block>消费笔数</Text>
          <Text weight="semibold" style={{ color: textColor }}>{data.currentCount}笔</Text>
          <Text size={100} style={{ color: mutedColor }} block>
            上月: {data.lastCount}笔
          </Text>
        </div>
      </div>

      <div style={{ marginTop: 12 }}>
        <div
          style={{
            height: 6,
            borderRadius: 3,
            background: theme === 'dark' ? '#444' : '#e0e0e0',
            position: 'relative',
            overflow: 'hidden',
          }}
        >
          <div
            style={{
              height: '100%',
              width: `${Math.min(Math.abs(data.changePercent), 100)}%`,
              borderRadius: 3,
              background: changeColor,
              position: 'absolute',
              left: isUp ? '50%' : `${50 - Math.min(Math.abs(data.changePercent), 100) / 2}%`,
              transition: 'width 0.3s ease',
            }}
          />
        </div>
        <Text size={100} style={{ color: mutedColor, marginTop: 4, display: 'block', textAlign: 'center' }}>
          环比变化: {changeArrow} {Math.abs(data.changePercent).toFixed(1)}%
        </Text>
      </div>
    </div>
  );
};
