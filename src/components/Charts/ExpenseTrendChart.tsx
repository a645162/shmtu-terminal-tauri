import React from 'react';
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
}

export const ExpenseTrendChart: React.FC<Props> = ({ data }) => {
  const theme = useAppStore((s) => s.theme);

  const chartData = data ?? [];
  const textColor = theme === 'dark' ? '#e0e0e0' : '#333';
  const gridColor = theme === 'dark' ? '#444' : '#eee';

  if (chartData.length === 0) {
    return (
      <div style={{ height: 200, display: 'flex', alignItems: 'center', justifyContent: 'center', color: 'var(--colorNeutralForeground3)' }}>
        暂无趋势数据
      </div>
    );
  }

  return (
    <ResponsiveContainer width="100%" height={200}>
      <LineChart data={chartData} margin={{ top: 5, right: 20, left: 0, bottom: 5 }}>
        <CartesianGrid strokeDasharray="3 3" stroke={gridColor} />
        <XAxis dataKey="date" tick={{ fontSize: 12, fill: textColor }} />
        <YAxis tick={{ fontSize: 12, fill: textColor }} />
        <Tooltip
          contentStyle={{
            backgroundColor: theme === 'dark' ? '#333' : '#fff',
            border: '1px solid #ccc',
            borderRadius: 4,
            color: textColor,
          }}
        />
        <Legend />
        <Line
          type="monotone"
          dataKey="expense"
          name="消费"
          stroke="#D13438"
          strokeWidth={2}
          dot={{ r: 3 }}
          activeDot={{ r: 5 }}
        />
        <Line
          type="monotone"
          dataKey="income"
          name="充值"
          stroke="#107C10"
          strokeWidth={2}
          dot={{ r: 3 }}
        />
      </LineChart>
    </ResponsiveContainer>
  );
};
