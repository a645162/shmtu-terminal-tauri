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
import type { MerchantRankingItem } from '../../types';

interface Props {
  data?: MerchantRankingItem[];
}

export const MerchantRankingChart: React.FC<Props> = ({ data }) => {
  const theme = useAppStore((s) => s.theme);

  const chartData = (data ?? []).slice(0, 10); // Top 10 merchants
  const textColor = theme === 'dark' ? '#e0e0e0' : '#333';
  const gridColor = theme === 'dark' ? '#444' : '#eee';

  if (chartData.length === 0) {
    return (
      <div style={{ height: 200, display: 'flex', alignItems: 'center', justifyContent: 'center', color: 'var(--colorNeutralForeground3)' }}>
        暂无商户数据
      </div>
    );
  }

  return (
    <ResponsiveContainer width="100%" height={200}>
      <BarChart data={chartData} layout="vertical" margin={{ top: 5, right: 20, left: 60, bottom: 5 }}>
        <CartesianGrid strokeDasharray="3 3" stroke={gridColor} />
        <XAxis type="number" tick={{ fontSize: 12, fill: textColor }} />
        <YAxis type="category" dataKey="merchant" tick={{ fontSize: 11, fill: textColor }} width={70} />
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
        <Bar dataKey="amount" name="amount" fill="#107C10" radius={[0, 4, 4, 0]}>
          {chartData.map((_, index) => (
            <Cell key={`cell-${index}`} fill={`hsl(120, 60%, ${50 - index * 4}%)`} />
          ))}
        </Bar>
      </BarChart>
    </ResponsiveContainer>
  );
};