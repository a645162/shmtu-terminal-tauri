import React, { useMemo } from 'react';
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

const MOCK_DATA = [
  { name: '早餐', amount: 45, count: 12 },
  { name: '午餐', amount: 220, count: 28 },
  { name: '晚餐', amount: 180, count: 24 },
  { name: '夜宵', amount: 35, count: 5 },
];

export const MealDistChart: React.FC = () => {
  const theme = useAppStore((s) => s.theme);
  const textColor = theme === 'dark' ? '#e0e0e0' : '#333';
  const gridColor = theme === 'dark' ? '#444' : '#eee';

  return (
    <ResponsiveContainer width="100%" height={250}>
      <BarChart data={MOCK_DATA} margin={{ top: 5, right: 20, left: 0, bottom: 5 }}>
        <CartesianGrid strokeDasharray="3 3" stroke={gridColor} />
        <XAxis dataKey="name" tick={{ fontSize: 12, fill: textColor }} />
        <YAxis tick={{ fontSize: 12, fill: textColor }} />
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
