import { useState, useEffect, useCallback } from 'react';
import { useAppStore } from '../stores/appStore';

export function useTheme() {
  const theme = useAppStore((s) => s.theme);
  const setTheme = useAppStore((s) => s.setTheme);

  const resolvedTheme = theme === 'system'
    ? (window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light')
    : theme;

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', resolvedTheme);
  }, [resolvedTheme]);

  const toggleTheme = useCallback(() => {
    setTheme(resolvedTheme === 'dark' ? 'light' : 'dark');
  }, [resolvedTheme, setTheme]);

  return { theme: resolvedTheme, setTheme, toggleTheme };
}

export function useDebounce<T>(value: T, delay: number): T {
  const [debouncedValue, setDebouncedValue] = useState(value);

  useEffect(() => {
    const timer = setTimeout(() => setDebouncedValue(value), delay);
    return () => clearTimeout(timer);
  }, [value, delay]);

  return debouncedValue;
}

export function usePageSize(defaultSize = 50) {
  const [pageSize, setPageSize] = useState(defaultSize);
  return { pageSize, setPageSize };
}

export function formatDate(dateStr: string): string {
  if (!dateStr) return '';
  return dateStr;
}

export function formatMoney(money: number): string {
  const abs = Math.abs(money);
  const formatted = abs.toFixed(2);
  if (money > 0) return `+${formatted}`;
  if (money < 0) return `-${formatted}`;
  return formatted;
}

export function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
}
