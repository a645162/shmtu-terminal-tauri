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
  ConsumptionBucketItem,
  MerchantRankingItem,
} from '../types';
import type { AppConfig } from '../services/tauri';
import * as tauri from '../services/tauri';
import { formatLocalDate } from '../utils/date';
import { initTranslationData } from '../utils/translation';

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
  consumptionDistribution: ConsumptionBucketItem[];
  merchantRanking: MerchantRankingItem[];
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
  // Error dialog state
  showErrorDialog: boolean;
  errorMessage: string;

  // Actions
  setCurrentIdentity: (identity: Identity | null) => void;
  activateIdentity: (identity: Identity) => Promise<void>;
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
  showError: (message: string) => void;
  setShowErrorDialog: (show: boolean) => void;
  loadStatisticsSummary: (params: tauri.StatisticsParams) => Promise<void>;
  loadTodaySummary: (params: tauri.StatisticsParams) => Promise<void>;
  loadMonthSummary: (params: tauri.StatisticsParams) => Promise<void>;
  loadDailyTrend: (params: tauri.StatisticsParams) => Promise<void>;
  loadCategoryDistribution: (params: tauri.StatisticsParams) => Promise<void>;
  loadMealDistribution: (params: tauri.StatisticsParams) => Promise<void>;
  loadConsumptionDistribution: (params: tauri.StatisticsParams) => Promise<void>;
  loadMerchantRanking: (params: tauri.StatisticsParams) => Promise<void>;
  refreshStatistics: () => Promise<void>;
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
  consumptionDistribution: [],
  merchantRanking: [],
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
  showErrorDialog: false,
  errorMessage: '',

  setCurrentIdentity: (identity) => set({ currentIdentity: identity }),

  activateIdentity: async (identity) => {
    set({ currentIdentity: identity });
    try {
      await tauri.set_last_identity(identity.id);
    } catch (e) {
      console.error('Failed to persist last identity:', e);
    }
    await get().loadAccounts(identity.id);
    await get().loadBills();
  },

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
      // 从后端加载最新分类规则（本地/GitHub），覆盖前端默认值
      initTranslationData(() => tauri.get_classification_rules()).catch(() => {});
    } catch (e) {
      console.error('Failed to load config:', e);
    }
  },

  setTheme: (theme) => {
    const currentConfig = get().config;
    set({ theme });

    if (!currentConfig) return;

    const nextConfig = {
      ...currentConfig,
      ui: {
        ...currentConfig.ui,
        theme,
      },
    };

    set({ config: nextConfig });
    tauri.save_config(nextConfig).catch(console.error);
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
      // 同步完成后刷新统计数据
      get().refreshStatistics();
    } catch (e) {
      get().showError(`同步失败: ${e}`);
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

  showError: (message) => {
    console.error('[App Error]', message);
    set({ showErrorDialog: true, errorMessage: message });
    // 发送错误到后端记录
    tauri.log_error(message).catch(console.error);
  },

  setShowErrorDialog: (show) => set({ showErrorDialog: show }),

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

  loadConsumptionDistribution: async (params) => {
    try {
      const distribution = await tauri.get_consumption_distribution(params);
      set({ consumptionDistribution: distribution });
    } catch (e) {
      console.error('Failed to load consumption distribution:', e);
      set({ consumptionDistribution: [] });
    }
  },

  loadMerchantRanking: async (params) => {
    try {
      const ranking = await tauri.get_merchant_ranking(params);
      set({ merchantRanking: ranking });
    } catch (e) {
      console.error('Failed to load merchant ranking:', e);
      set({ merchantRanking: [] });
    }
  },

  refreshStatistics: async () => {
    const { currentIdentity } = get();
    if (!currentIdentity) return;
    const params = { identityId: currentIdentity.id };

    try {
      const summary = await tauri.get_statistics_summary(params);
      set({ statisticsSummary: summary });
    } catch (e) {
      console.error('Failed to refresh statistics summary:', e);
    }

    // 今日统计
    const today = formatLocalDate(new Date());
    try {
      const todaySummary = await tauri.get_statistics_summary({ identityId: currentIdentity.id, dateStart: today, dateEnd: today });
      set({ todaySummary });
    } catch (e) {
      console.error('Failed to refresh today summary:', e);
    }

    // 本月统计
    const now = new Date();
    const monthStart = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}-01`;
    try {
      const monthSummary = await tauri.get_statistics_summary({ identityId: currentIdentity.id, dateStart: monthStart });
      set({ monthSummary });
    } catch (e) {
      console.error('Failed to refresh month summary:', e);
    }

    // 每日趋势（最近30天）
    const trendEnd = today;
    const trendStart = formatLocalDate(new Date(now.getTime() - 30 * 24 * 60 * 60 * 1000));
    try {
      const dailyTrend = await tauri.get_daily_trend({ identityId: currentIdentity.id, dateStart: trendStart, dateEnd: trendEnd });
      set({ dailyTrend });
    } catch (e) {
      console.error('Failed to refresh daily trend:', e);
    }

    // 分类分布
    try {
      const categoryDistribution = await tauri.get_category_distribution(params);
      set({ categoryDistribution });
    } catch (e) {
      console.error('Failed to refresh category distribution:', e);
    }

    // 餐饮分布
    try {
      const mealDistribution = await tauri.get_meal_distribution(params);
      set({ mealDistribution });
    } catch (e) {
      console.error('Failed to refresh meal distribution:', e);
    }

    // 消费分布
    try {
      const consumptionDistribution = await tauri.get_consumption_distribution(params);
      set({ consumptionDistribution });
    } catch (e) {
      console.error('Failed to refresh consumption distribution:', e);
    }

    // 商户排行
    try {
      const merchantRanking = await tauri.get_merchant_ranking(params);
      set({ merchantRanking });
    } catch (e) {
      console.error('Failed to refresh merchant ranking:', e);
    }
  },
}));
