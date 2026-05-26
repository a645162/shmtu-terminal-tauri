import React from 'react';
import {
  BarChart,
  Bar,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  Legend,
} from 'recharts';
import { useAppStore } from '../../stores/appStore';
import type { MealDistItem } from '../../types';

interface Props {
  data?: MealDistItem[];
}

export const MealDistChart: React.FC<Props> = ({ data }) => {
  const theme = useAppStore((s) => s.theme);

  const chartData = data ?? [];
  const textColor = theme === 'dark' ? '#e0e0e0' : '#333';
  const gridColor = theme === 'dark' ? '#444' : '#eee';

  if (chartData.length === 0) {
    return (
      <div style={{ height: 250, display: 'flex', alignItems: 'center', justifyContent: 'center', color: 'var(--colorNeutralForeground3)' }}>
        暂无用餐数据
      </div>
    );
  }

  return (
    <ResponsiveContainer width="100%" height={250}>
      <BarChart data={chartData} margin={{ top: 5, right: 20, left: 0, bottom: 5 }}>
        <CartesianGrid strokeDasharray="3 3" stroke={gridColor} />
        <XAxis dataKey="name" tick={{ fontSize: 12, fill: textColor }} />
        <YAxis tick={{ fontSize: 12, fill: textColor }} tickFormatter={(v: number) => v.toFixed(2)} />
        <Tooltip
          contentStyle={{
            backgroundColor: theme === 'dark' ? '#333' : '#fff',
            border: '1px solid #ccc',
            borderRadius: 4,
            color: textColor,
          }}
          formatter={(value: unknown, name: unknown) =>
            name === 'amount' ? `¥${Number(value).toFixed(2)}` : `${value}次`
          }
        />
        <Legend />
        <Bar dataKey="amount" name="消费金额" fill="#0078D4" radius={[4, 4, 0, 0]} />
        <Bar dataKey="count" name="消费次数" fill="#FF8C00" radius={[4, 4, 0, 0]} />
      </BarChart>
    </ResponsiveContainer>
  );
};
