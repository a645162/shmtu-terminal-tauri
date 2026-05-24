import React from 'react';
import {
  BarChart,
  Bar,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  Cell,
} from 'recharts';
import { useAppStore } from '../../stores/appStore';
import type { ConsumptionBucketItem } from '../../types';

interface Props {
  data?: ConsumptionBucketItem[];
}

export const ConsumptionDistributionChart: React.FC<Props> = ({ data }) => {
  const theme = useAppStore((s) => s.theme);

  const chartData = data ?? [];
  const textColor = theme === 'dark' ? '#e0e0e0' : '#333';
  const gridColor = theme === 'dark' ? '#444' : '#eee';

  if (chartData.length === 0) {
    return (
      <div style={{ height: 200, display: 'flex', alignItems: 'center', justifyContent: 'center', color: 'var(--colorNeutralForeground3)' }}>
        暂无消费分布数据
      </div>
    );
  }

  return (
    <ResponsiveContainer width="100%" height={200}>
      <BarChart data={chartData} margin={{ top: 5, right: 20, left: 0, bottom: 5 }}>
        <CartesianGrid strokeDasharray="3 3" stroke={gridColor} />
        <XAxis dataKey="range" tick={{ fontSize: 11, fill: textColor }} interval={0} />
        <YAxis tick={{ fontSize: 12, fill: textColor }} />
        <Tooltip
          contentStyle={{
            backgroundColor: theme === 'dark' ? '#333' : '#fff',
            border: '1px solid #ccc',
            borderRadius: 4,
            color: textColor,
          }}
          formatter={(value: unknown, name: unknown) => {
            if (String(name) === 'count') return [`${value} 笔`, '消费笔数'];
            return [`¥${Number(value).toFixed(2)}`, '消费金额'];
          }}
        />
        <Bar dataKey="amount" name="amount" fill="#0078D4" radius={[4, 4, 0, 0]}>
          {chartData.map((_, index) => (
            <Cell key={`cell-${index}`} fill={`hsl(210, 80%, ${60 - index * 8}%)`} />
          ))}
        </Bar>
      </BarChart>
    </ResponsiveContainer>
  );
};