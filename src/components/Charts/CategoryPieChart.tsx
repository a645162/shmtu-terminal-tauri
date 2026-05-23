import React from 'react';
import {
  PieChart,
  Pie,
  Cell,
  Tooltip,
  ResponsiveContainer,
  Legend,
} from 'recharts';
import { useAppStore } from '../../stores/appStore';
import type { CategoryItem } from '../../types';

const COLORS = ['#0078D4', '#FF8C00', '#107C10', '#D13438', '#8764B8', '#00B7C3', '#FFB900'];

interface Props {
  data?: CategoryItem[];
}

export const CategoryPieChart: React.FC<Props> = ({ data }) => {
  const theme = useAppStore((s) => s.theme);

  const chartData = data ?? [];
  const textColor = theme === 'dark' ? '#e0e0e0' : '#333';

  if (chartData.length === 0) {
    return (
      <div style={{ height: 200, display: 'flex', alignItems: 'center', justifyContent: 'center', color: 'var(--colorNeutralForeground3)' }}>
        暂无分类数据
      </div>
    );
  }

  return (
    <ResponsiveContainer width="100%" height={200}>
      <PieChart>
        <Pie
          data={chartData}
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
          {chartData.map((_, index) => (
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
