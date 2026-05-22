import React, { useMemo } from 'react';
import {
  PieChart,
  Pie,
  Cell,
  Tooltip,
  ResponsiveContainer,
  Legend,
} from 'recharts';
import { useAppStore } from '../../stores/appStore';

const COLORS = ['#0078D4', '#FF8C00', '#107C10', '#D13438', '#8764B8', '#00B7C3', '#FFB900'];

const MOCK_DATA = [
  { name: '食堂', value: 450, count: 56 },
  { name: '充值', value: 500, count: 2 },
  { name: '热水', value: 80, count: 20 },
  { name: '洗澡', value: 60, count: 15 },
  { name: '电费', value: 30, count: 1 },
  { name: '其他', value: 50, count: 8 },
];

export const CategoryPieChart: React.FC = () => {
  const bills = useAppStore((s) => s.bills);
  const theme = useAppStore((s) => s.theme);

  const data = useMemo(() => {
    // When backend is ready, compute from real data with classification rules
    if (bills.length === 0) return MOCK_DATA;
    return MOCK_DATA; // Fallback to mock
  }, [bills]);

  const textColor = theme === 'dark' ? '#e0e0e0' : '#333';

  return (
    <ResponsiveContainer width="100%" height={200}>
      <PieChart>
        <Pie
          data={data}
          cx="50%"
          cy="50%"
          innerRadius={45}
          outerRadius={70}
          paddingAngle={2}
          dataKey="value"
          nameKey="name"
          label={({ name, percent }: any) => `${name} ${((percent ?? 0) * 100).toFixed(0)}%`}
          labelLine={{ stroke: textColor }}
        >
          {data.map((_, index) => (
            <Cell key={`cell-${index}`} fill={COLORS[index % COLORS.length]} />
          ))}
        </Pie>
        <Tooltip
          contentStyle={{
            backgroundColor: theme === 'dark' ? '#333' : '#fff',
            border: '1px solid #ccc',
            borderRadius: 4,
            color: textColor,
          }}
          formatter={(value: unknown) => `¥${Number(value).toFixed(2)}`}
        />
      </PieChart>
    </ResponsiveContainer>
  );
};
