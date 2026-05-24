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
  LabelList,
} from 'recharts';
import { useAppStore } from '../../stores/appStore';
import type { CategoryItem } from '../../types';
import { getCategoryDisplayName, getCategoryColor } from '../../utils/translation';

interface Props {
  data?: CategoryItem[];
}

export const CategoryBarChart: React.FC<Props> = ({ data }) => {
  const theme = useAppStore((s) => s.theme);

  const chartData = (data ?? [])
    .filter((item) => item.value > 0)
    .map((item) => ({
      ...item,
      displayName: getCategoryDisplayName(item.name),
      color: getCategoryColor(item.name),
    }))
    .sort((a, b) => b.value - a.value);

  const textColor = theme === 'dark' ? '#e0e0e0' : '#333';
  const gridColor = theme === 'dark' ? '#444' : '#eee';

  if (chartData.length === 0) {
    return (
      <div style={{ height: 260, display: 'flex', alignItems: 'center', justifyContent: 'center', color: 'var(--colorNeutralForeground3)' }}>
        暂无分类数据
      </div>
    );
  }

  return (
    <ResponsiveContainer width="100%" height={260}>
      <BarChart data={chartData} margin={{ top: 20, right: 20, left: 0, bottom: 5 }} layout="vertical">
        <CartesianGrid strokeDasharray="3 3" stroke={gridColor} />
        <XAxis type="number" tick={{ fontSize: 12, fill: textColor }} />
        <YAxis
          type="category"
          dataKey="displayName"
          tick={{ fontSize: 12, fill: textColor }}
          width={60}
        />
        <Tooltip
          contentStyle={{
            backgroundColor: theme === 'dark' ? '#333' : '#fff',
            border: '1px solid #ccc',
            borderRadius: 4,
            color: textColor,
          }}
          formatter={(value: unknown, _name: unknown, props: any) => [
            `¥${Number(value).toFixed(2)} (${props.payload.count}笔)`,
            props.payload.displayName,
          ]}
        />
        <Bar dataKey="value" name="金额" radius={[0, 4, 4, 0]} maxBarSize={32}>
          {chartData.map((entry, index) => (
            <Cell key={`cell-${index}`} fill={entry.color} />
          ))}
          <LabelList
            dataKey="value"
            position="right"
            formatter={(value) => `¥${Number(value).toFixed(2)}`}
            style={{ fontSize: 11, fill: textColor }}
          />
        </Bar>
      </BarChart>
    </ResponsiveContainer>
  );
};
