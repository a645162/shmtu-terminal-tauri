import React, { useMemo } from 'react';
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
import type { MerchantRankingItem } from '../../types';
import { groupMerchantsByPosition } from '../../utils/translation';

interface Props {
  data?: MerchantRankingItem[];
}

const POSITION_COLORS = [
  '#0078D4', '#FF8C00', '#107C10', '#D13438', '#8764B8',
  '#00B7C3', '#FFB900', '#E81123', '#0098BC', '#881798',
  '#498205', '#8E8E8E', '#C239B3', '#00CC6A', '#F7630C',
];

const renderActiveShape = (props: any) => {
  const {
    cx, cy, innerRadius, outerRadius, startAngle, endAngle,
    fill, payload, percent, value,
  } = props;

  return (
    <g>
      <text x={cx} y={cy - 8} textAnchor="middle" fill={fill} fontSize={14} fontWeight="bold">
        {payload.position}
      </text>
      <text x={cx} y={cy + 12} textAnchor="middle" fill={fill} fontSize={12}>
        ¥{value.toFixed(2)}
      </text>
      <text x={cx} y={cy + 28} textAnchor="middle" fill="#888" fontSize={11}>
        {`(${(percent * 100).toFixed(1)}%)`}
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

export const PositionPieChart: React.FC<Props> = ({ data }) => {
  const theme = useAppStore((s) => s.theme);

  const chartData = useMemo(() => {
    if (!data || data.length === 0) return [];
    const merchants = data.map((item) => ({
      merchant: item.merchant,
      amount: item.amount,
      count: item.count,
    }));
    return groupMerchantsByPosition(merchants);
  }, [data]);

  const textColor = theme === 'dark' ? '#e0e0e0' : '#333';

  if (chartData.length === 0) {
    return (
      <div style={{ height: 260, display: 'flex', alignItems: 'center', justifyContent: 'center', color: 'var(--colorNeutralForeground3)' }}>
        暂无位置数据
      </div>
    );
  }

  return (
    <ResponsiveContainer width="100%" height={260}>
      <PieChart>
        <Pie
          data={chartData}
          cx="50%"
          cy="50%"
          innerRadius={55}
          outerRadius={80}
          paddingAngle={2}
          dataKey="amount"
          nameKey="position"
          activeShape={renderActiveShape}
        >
          {chartData.map((_, index) => (
            <Cell key={`cell-${index}`} fill={POSITION_COLORS[index % POSITION_COLORS.length]} />
          ))}
        </Pie>
        <Tooltip
          contentStyle={{
            backgroundColor: theme === 'dark' ? '#333' : '#fff',
            border: '1px solid #ccc',
            borderRadius: 4,
            color: textColor,
          }}
          formatter={(value, _name, props) => [
            `¥${Number(value).toFixed(2)} (${props.payload.count}笔)`,
            props.payload.position,
          ]}
        />
        <Legend
          formatter={(value: string) => (
            <span style={{ color: textColor, fontSize: 12 }}>{value}</span>
          )}
        />
      </PieChart>
    </ResponsiveContainer>
  );
};
