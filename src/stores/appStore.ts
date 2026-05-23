import { create } from 'zustand';
import type {
  Identity,
  Account,
  BillItem,
  BillType,
  SyncProgress,
  AppTheme,
  StatisticsSummary,
  DailyTrendItem,
  CategoryItem,
  MealDistItem,
} from '../types';
import type { AppConfig } from '../services/tauri';
import * as tauri from '../services/tauri';

// ========== App Store ==========

interface AppState {
  // Current identity
  currentIdentity: Identity | null;
  identities: Identity[];
  accounts: Account[];

  // Bills
  bills: BillItem[];
  billTotal: number;
  billPage: number;
  billPageSize: number;
  billType: BillType;
  billKeyword: string;
  billDateStart: string;
  billDateEnd: string;

  // Sync
  syncProgress: SyncProgress | null;

  // Config
  config: AppConfig | null;
  theme: AppTheme;

  // Statistics
  statisticsSummary: StatisticsSummary | null;
  todaySummary: StatisticsSummary | null;
  monthSummary: StatisticsSummary | null;
  dailyTrend: DailyTrendItem[];
  categoryDistribution: CategoryItem[];
  mealDistribution: MealDistItem[];
  isLoadingStatistics: boolean;

  // UI state
  isLoading: boolean;
  showStartupDialog: boolean;
  showIdentitySelectDialog: boolean;
  showIdentityManagerDialog: boolean;
  showSettingsDialog: boolean;
  showAboutDialog: boolean;
  showCaptchaTestDialog: boolean;
  showDataTransferDialog: boolean;
  showStatisticsDialog: boolean;
  showManualCaptchaDialog: boolean;
  captchaImage: string | null;
  captchaExecution: string | null;

  // Actions
  setCurrentIdentity: (identity: Identity | null) => void;
  loadIdentities: () => Promise<void>;
  loadAccounts: (identityId: number) => Promise<void>;
  loadBills: () => Promise<void>;
  setBillPage: (page: number) => void;
  setBillType: (type: BillType) => void;
  setBillKeyword: (keyword: string) => void;
  setBillDateRange: (start: string, end: string) => void;
  setBillPageSize: (size: number) => void;
  loadConfig: () => Promise<void>;
  setTheme: (theme: AppTheme) => void;
  startSync: (identityId: number) => Promise<void>;
  setShowStartupDialog: (show: boolean) => void;
  setShowIdentitySelectDialog: (show: boolean) => void;
  setShowIdentityManagerDialog: (show: boolean) => void;
  setShowSettingsDialog: (show: boolean) => void;
  setShowAboutDialog: (show: boolean) => void;
  setShowCaptchaTestDialog: (show: boolean) => void;
  setShowDataTransferDialog: (show: boolean) => void;
  setShowStatisticsDialog: (show: boolean) => void;
  setShowManualCaptchaDialog: (show: boolean) => void;
  setCaptchaForManualLogin: (image: string | null, execution: string | null) => void;
  loadStatisticsSummary: (params: tauri.StatisticsParams) => Promise<void>;
  loadTodaySummary: (params: tauri.StatisticsParams) => Promise<void>;
  loadMonthSummary: (params: tauri.StatisticsParams) => Promise<void>;
  loadDailyTrend: (params: tauri.StatisticsParams) => Promise<void>;
  loadCategoryDistribution: (params: tauri.StatisticsParams) => Promise<void>;
  loadMealDistribution: (params: tauri.StatisticsParams) => Promise<void>;
}

export const useAppStore = create<AppState>((set, get) => ({
  currentIdentity: null,
  identities: [],
  accounts: [],

  bills: [],
  billTotal: 0,
  billPage: 1,
  billPageSize: 50,
  billType: 'all',
  billKeyword: '',
  billDateStart: '',
  billDateEnd: '',

  syncProgress: null,

  config: null,
  theme: 'light',

  // Statistics
  statisticsSummary: null,
  todaySummary: null,
  monthSummary: null,
  dailyTrend: [],
  categoryDistribution: [],
  mealDistribution: [],
  isLoadingStatistics: false,

  isLoading: false,
  showStartupDialog: false,
  showIdentitySelectDialog: false,
  showIdentityManagerDialog: false,
  showSettingsDialog: false,
  showAboutDialog: false,
  showCaptchaTestDialog: false,
  showDataTransferDialog: false,
  showStatisticsDialog: false,
  showManualCaptchaDialog: false,
  captchaImage: null,
  captchaExecution: null,

  setCurrentIdentity: (identity) => set({ currentIdentity: identity }),

  loadIdentities: async () => {
    try {
      const identities = await tauri.list_identities();
      set({ identities });
    } catch (e) {
      console.error('Failed to load identities:', e);
    }
  },

  loadAccounts: async (identityId) => {
    try {
      const accounts = await tauri.list_accounts(identityId);
      set({ accounts });
    } catch (e) {
      console.error('Failed to load accounts:', e);
    }
  },

  loadBills: async (overrides?: { type?: BillType; keyword?: string; dateStart?: string; dateEnd?: string }) => {
    const { currentIdentity, billPage, billPageSize, billType, billKeyword, billDateStart, billDateEnd } = get();
    if (!currentIdentity) return;
    set({ isLoading: true });
    const type = overrides?.type ?? billType;
    const keyword = overrides?.keyword ?? billKeyword;
    const dateStart = overrides?.dateStart ?? billDateStart;
    const dateEnd = overrides?.dateEnd ?? billDateEnd;
    try {
      const result = await tauri.query_bills({
        identityId: currentIdentity.id,
        billType: type,
        page: billPage,
        pageSize: billPageSize,
        keyword: keyword || undefined,
        dateStart: dateStart || undefined,
        dateEnd: dateEnd || undefined,
      });
      set({ bills: result.items, billTotal: result.total, isLoading: false });
    } catch (e) {
      console.error('Failed to load bills:', e);
      set({ isLoading: false });
    }
  },

  setBillPage: (page) => {
    set({ billPage: page });
    get().loadBills();
  },

  setBillType: (type) => {
    set({ billType: type, billPage: 1 });
    get().loadBills();
  },

  setBillKeyword: (keyword) => {
    set({ billKeyword: keyword, billPage: 1 });
  },

  setBillDateRange: (start, end) => {
    set({ billDateStart: start, billDateEnd: end, billPage: 1 });
    get().loadBills();
  },

  setBillPageSize: (size) => {
    set({ billPageSize: size, billPage: 1 });
    get().loadBills();
  },

  loadConfig: async () => {
    try {
      const config = await tauri.load_config();
      set({ config, theme: config.ui.theme });
    } catch (e) {
      console.error('Failed to load config:', e);
    }
  },

  setTheme: (theme) => {
    set({ theme });
    tauri.save_config({ ui: { theme, language: 'zh-CN' } }).catch(console.error);
  },

  startSync: async (identityId) => {
    try {
      const progress = await tauri.incremental_sync(identityId);
      // 检查是否需要手动输入验证码
      if (progress.captcha_required && progress.captcha_image && progress.execution) {
        set({
          syncProgress: { ...progress, status: 'captcha_required' },
          captchaImage: progress.captcha_image,
          captchaExecution: progress.execution,
          showManualCaptchaDialog: true,
        });
        return;
      }
      set({ syncProgress: progress });
      get().loadBills();
    } catch (e) {
      console.error('Sync failed:', e);
    }
  },

  setShowStartupDialog: (show) => set({ showStartupDialog: show }),
  setShowIdentitySelectDialog: (show) => set({ showIdentitySelectDialog: show }),
  setShowIdentityManagerDialog: (show) => set({ showIdentityManagerDialog: show }),
  setShowSettingsDialog: (show) => set({ showSettingsDialog: show }),
  setShowAboutDialog: (show) => set({ showAboutDialog: show }),
  setShowCaptchaTestDialog: (show) => set({ showCaptchaTestDialog: show }),
  setShowDataTransferDialog: (show) => set({ showDataTransferDialog: show }),
  setShowStatisticsDialog: (show) => set({ showStatisticsDialog: show }),
  setShowManualCaptchaDialog: (show) =>
    set(
      show
        ? { showManualCaptchaDialog: true }
        : {
            showManualCaptchaDialog: false,
            captchaImage: null,
            captchaExecution: null,
          }
    ),
  setCaptchaForManualLogin: (image, execution) => set({ captchaImage: image, captchaExecution: execution, showManualCaptchaDialog: true }),

  loadStatisticsSummary: async (params) => {
    set({ isLoadingStatistics: true });
    try {
      const summary = await tauri.get_statistics_summary(params);
      set({ statisticsSummary: summary, isLoadingStatistics: false });
    } catch (e) {
      console.error('Failed to load statistics summary:', e);
      set({ statisticsSummary: null, isLoadingStatistics: false });
    }
  },
  loadTodaySummary: async (params) => {
    try {
      const summary = await tauri.get_statistics_summary(params);
      set({ todaySummary: summary });
    } catch (e) {
      console.error('Failed to load today summary:', e);
      set({ todaySummary: null });
    }
  },
  loadMonthSummary: async (params) => {
    try {
      const summary = await tauri.get_statistics_summary(params);
      set({ monthSummary: summary });
    } catch (e) {
      console.error('Failed to load month summary:', e);
      set({ monthSummary: null });
    }
  },

  loadDailyTrend: async (params) => {
    try {
      const trend = await tauri.get_daily_trend(params);
      set({ dailyTrend: trend });
    } catch (e) {
      console.error('Failed to load daily trend:', e);
      set({ dailyTrend: [] });
    }
  },

  loadCategoryDistribution: async (params) => {
    try {
      const distribution = await tauri.get_category_distribution(params);
      set({ categoryDistribution: distribution });
    } catch (e) {
      console.error('Failed to load category distribution:', e);
      set({ categoryDistribution: [] });
    }
  },

  loadMealDistribution: async (params) => {
    try {
      const distribution = await tauri.get_meal_distribution(params);
      set({ mealDistribution: distribution });
    } catch (e) {
      console.error('Failed to load meal distribution:', e);
      set({ mealDistribution: [] });
    }
  },
}));
