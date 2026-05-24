import React, { useCallback } from 'react';
import {
  PieChart,
  Pie,
  Cell,
  Tooltip,
  ResponsiveContainer,
  Legend,
  Sector,
} from 'recharts';
import { useAppStore } from '../../stores/appStore';
import type { CategoryItem } from '../../types';
import { getCategoryDisplayName, getCategoryColor } from '../../utils/translation';

const FALLBACK_COLORS = ['#0078D4', '#FF8C00', '#107C10', '#D13438', '#8764B8', '#00B7C3', '#FFB900'];

interface Props {
  data?: CategoryItem[];
  onCategoryClick?: (categoryName: string) => void;
}

const renderActiveShape = (props: any) => {
  const {
    cx, cy, innerRadius, outerRadius, startAngle, endAngle,
    fill, payload, percent, value, count,
  } = props;

  return (
    <g>
      <text x={cx} y={cy - 8} textAnchor="middle" fill={fill} fontSize={14} fontWeight="bold">
        {payload.displayName ?? payload.name}
      </text>
      <text x={cx} y={cy + 12} textAnchor="middle" fill={fill} fontSize={12}>
        ¥{value.toFixed(2)}
      </text>
      <text x={cx} y={cy + 28} textAnchor="middle" fill="#888" fontSize={11}>
        {`${count}笔 (${(percent * 100).toFixed(1)}%)`}
      </text>
      <Sector
        cx={cx}
        cy={cy}
        innerRadius={innerRadius - 2}
        outerRadius={outerRadius + 6}
        startAngle={startAngle}
        endAngle={endAngle}
        fill={fill}
      />
      <Sector
        cx={cx}
        cy={cy}
        innerRadius={outerRadius + 8}
        outerRadius={outerRadius + 12}
        startAngle={startAngle}
        endAngle={endAngle}
        fill={fill}
      />
    </g>
  );
};

export const CategoryPieChart: React.FC<Props> = ({ data, onCategoryClick }) => {
  const theme = useAppStore((s) => s.theme);
  const handleClick = useCallback(
    (entry: any) => {
      if (onCategoryClick && entry?.name) {
        onCategoryClick(entry.name);
      }
    },
    [onCategoryClick]
  );

  const chartData = (data ?? []).map((item) => ({
    ...item,
    displayName: getCategoryDisplayName(item.name),
    color: getCategoryColor(item.name),
  }));

  const textColor = theme === 'dark' ? '#e0e0e0' : '#333';

  if (chartData.length === 0) {
    return (
      <div style={{ height: 220, display: 'flex', alignItems: 'center', justifyContent: 'center', color: 'var(--colorNeutralForeground3)' }}>
        暂无分类数据
      </div>
    );
  }

  return (
    <ResponsiveContainer width="100%" height={220}>
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
          activeShape={renderActiveShape}
          onClick={handleClick}
          style={{ cursor: onCategoryClick ? 'pointer' : 'default' }}
        >
          {chartData.map((entry, index) => (
            <Cell key={`cell-${index}`} fill={entry.color ?? FALLBACK_COLORS[index % FALLBACK_COLORS.length]} />
          ))}
        </Pie>
        <Tooltip
          contentStyle={{
            backgroundColor: theme === 'dark' ? '#333' : '#fff',
            border: '1px solid #ccc',
            borderRadius: 4,
            color: textColor,
          }}
          formatter={(value: unknown, _name: unknown, props: any) => [
            `¥${Number(value).toFixed(2)} (${props.payload.count}笔)`,
            props.payload.displayName ?? props.payload.name,
          ]}
        />
        <Legend
          formatter={(value: string, _entry: any, index: number) => {
            const item = chartData[index];
            return (
              <span style={{ color: textColor, fontSize: 12 }}>
                {item?.displayName ?? value}
              </span>
            );
          }}
        />
      </PieChart>
    </ResponsiveContainer>
  );
};
