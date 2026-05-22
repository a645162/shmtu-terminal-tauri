import React, { useMemo } from 'react';
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

// Generate mock 7-day trend data when no real data is available
const generateMockTrend = () => {
  const days = ['周一', '周二', '周三', '周四', '周五', '周六', '周日'];
  return days.map((day) => ({
    date: day,
    expense: Math.round(Math.random() * 40 + 10),
    income: Math.random() > 0.7 ? Math.round(Math.random() * 200 + 50) : 0,
  }));
};

export const ExpenseTrendChart: React.FC = () => {
  const bills = useAppStore((s) => s.bills);
  const theme = useAppStore((s) => s.theme);

  const data = useMemo(() => {
    // When backend is ready, compute from real bills
    // For now use mock data
    if (bills.length === 0) {
      return generateMockTrend();
    }
    // Try to compute from real data
    const dayMap = new Map<string, { expense: number; income: number }>();
    const today = new Date();
    for (let i = 6; i >= 0; i--) {
      const d = new Date(today);
      d.setDate(d.getDate() - i);
      const key = d.toLocaleDateString('zh-CN', { weekday: 'short' });
      dayMap.set(key, { expense: 0, income: 0 });
    }
    bills.forEach((bill) => {
      // Simple approximation
      if (bill.money < 0) {
        const existing = Array.from(dayMap.values()).reduce((a, b) => a + b.expense, 0);
        if (existing === 0) {
          // No matching date, use mock
        }
      }
    });
    // Return mock if we can't properly compute
    return generateMockTrend();
  }, [bills]);

  const textColor = theme === 'dark' ? '#e0e0e0' : '#333';
  const gridColor = theme === 'dark' ? '#444' : '#eee';

  return (
    <ResponsiveContainer width="100%" height={200}>
      <LineChart data={data} margin={{ top: 5, right: 20, left: 0, bottom: 5 }}>
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
