import React, { useState, useCallback } from 'react';
import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  Legend,
} from 'recharts';
import { useAppStore } from '../../stores/appStore';
import type { DailyTrendItem } from '../../types';

interface Props {
  data?: DailyTrendItem[];
  onDateClick?: (date: string) => void;
}

export const ExpenseTrendChart: React.FC<Props> = ({ data, onDateClick }) => {
  const theme = useAppStore((s) => s.theme);
  const [hiddenLines, setHiddenLines] = useState<Set<string>>(new Set());
  const handleClick = useCallback(
    (data: any) => {
      if (onDateClick && data?.activeLabel) {
        onDateClick(data.activeLabel);
      }
    },
    [onDateClick]
  );

  const chartData = data ?? [];
  const textColor = theme === 'dark' ? '#e0e0e0' : '#333';
  const gridColor = theme === 'dark' ? '#444' : '#eee';

  const toggleLine = useCallback((dataKey: string) => {
    setHiddenLines((prev) => {
      const next = new Set(prev);
      if (next.has(dataKey)) {
        next.delete(dataKey);
      } else {
        next.add(dataKey);
      }
      return next;
    });
  }, []);

  if (chartData.length === 0) {
    return (
      <div style={{ height: 220, display: 'flex', alignItems: 'center', justifyContent: 'center', color: 'var(--colorNeutralForeground3)' }}>
        暂无趋势数据
      </div>
    );
  }

  return (
    <ResponsiveContainer width="100%" height={220}>
      <LineChart
        data={chartData}
        margin={{ top: 5, right: 20, left: 0, bottom: 5 }}
        onClick={handleClick}
        style={{ cursor: onDateClick ? 'pointer' : 'default' }}
      >
        <CartesianGrid strokeDasharray="3 3" stroke={gridColor} />
        <XAxis dataKey="date" tick={{ fontSize: 11, fill: textColor }} />
        <YAxis tick={{ fontSize: 11, fill: textColor }} />
        <Tooltip
          contentStyle={{
            backgroundColor: theme === 'dark' ? '#333' : '#fff',
            border: '1px solid #ccc',
            borderRadius: 4,
            color: textColor,
          }}
          formatter={(value, name) => {
            if (name === 'expense') return [`¥${Number(value).toFixed(2)}`, '消费'];
            if (name === 'income') return [`¥${Number(value).toFixed(2)}`, '充值'];
            return [String(value ?? ''), String(name ?? '')];
          }}
          labelFormatter={(label) => String(label ?? '')}
        />
        <Legend
          onClick={(e: any) => {
            const dataKey = e.dataKey;
            if (dataKey) toggleLine(dataKey);
          }}
          formatter={(value: string, _entry: any) => (
            <span style={{ color: textColor, fontSize: 12, cursor: 'pointer' }}>
              {value}
            </span>
          )}
        />
        <Line
          type="monotone"
          dataKey="expense"
          name="消费"
          stroke="#D13438"
          strokeWidth={2}
          dot={{ r: 3 }}
          activeDot={{ r: 6 }}
          hide={hiddenLines.has('expense')}
        />
        <Line
          type="monotone"
          dataKey="income"
          name="充值"
          stroke="#107C10"
          strokeWidth={2}
          dot={{ r: 3 }}
          activeDot={{ r: 6 }}
          hide={hiddenLines.has('income')}
        />
      </LineChart>
    </ResponsiveContainer>
  );
};
