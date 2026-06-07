import { create } from 'zustand';
import type {
  Identity,
  Account,
  BillItem,
  BillType,
  SyncProgress,
  SyncRangePreset,
  AppTheme,
  AppSettingsTab,
  StatisticsSummary,
  DailyTrendItem,
  CategoryItem,
  MealDistItem,
  ConsumptionBucketItem,
  MerchantRankingItem,
} from '../types';
import type { AppConfig, P2PStatus, P2PTransferProgress, P2PPairingRequest } from '../services/tauri';
import * as tauri from '../services/tauri';
import { formatLocalDate } from '../utils/date';
import { initTranslationData } from '../utils/translation';

// ========== App Store ==========

type PendingSyncAction =
  | { kind: 'identity_incremental'; identityId: number }
  | { kind: 'identity_full'; identityId: number }
  | { kind: 'account_incremental'; identityId: number; accountId: string }
  | { kind: 'account_full'; identityId: number; accountId: string };

function syncRangeLabel(syncRange: SyncRangePreset): string {
  switch (syncRange) {
    case 'week':
      return '最近一周';
    case 'half_month':
      return '最近半个月';
    case 'month':
      return '最近一个月';
    case 'half_year':
      return '最近半年';
    case 'year':
      return '最近一年';
    case 'all':
      return '全部';
  }
}

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
  forgotCardStats: tauri.ForgotCardStats | null;
  isLoadingStatistics: boolean;

  // UI state
  isLoading: boolean;
  showStartupDialog: boolean;
  showIdentitySelectDialog: boolean;
  showIdentityManagerDialog: boolean;
  showSettingsDialog: boolean;
  settingsDialogTab: AppSettingsTab;
  showAboutDialog: boolean;
  showCaptchaTestDialog: boolean;
  showDataTransferDialog: boolean;
  showStatisticsDialog: boolean;
  showSyncRangeDialog: boolean;
  showManualCaptchaDialog: boolean;
  captchaImage: string | null;
  captchaExecution: string | null;
  pendingSyncAction: PendingSyncAction | null;
  // Error dialog state
  showErrorDialog: boolean;
  errorMessage: string;

  // P2P Transfer state
  showP2PDialog: boolean;
  p2pStatus: P2PStatus | null;
  p2pTransferProgress: P2PTransferProgress | null;
  pendingPairRequest: P2PPairingRequest | null;

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
  startSync: (identityId: number, syncRange: SyncRangePreset) => Promise<void>;
  startFullSync: (identityId: number, syncRange: SyncRangePreset) => Promise<void>;
  startSyncAccount: (identityId: number, accountId: string, syncRange: SyncRangePreset) => Promise<void>;
  startFullSyncAccount: (identityId: number, accountId: string, syncRange: SyncRangePreset) => Promise<void>;
  openSyncRangeDialog: (action: PendingSyncAction) => void;
  closeSyncRangeDialog: () => void;
  confirmSyncRange: (syncRange: SyncRangePreset) => Promise<void>;
  setShowStartupDialog: (show: boolean) => void;
  setShowIdentitySelectDialog: (show: boolean) => void;
  setShowIdentityManagerDialog: (show: boolean) => void;
  setShowSettingsDialog: (show: boolean) => void;
  openSettingsDialog: (tab?: AppSettingsTab) => void;
  setShowAboutDialog: (show: boolean) => void;
  setShowCaptchaTestDialog: (show: boolean) => void;
  setShowDataTransferDialog: (show: boolean) => void;
  setShowStatisticsDialog: (show: boolean) => void;
  setShowManualCaptchaDialog: (show: boolean) => void;
  setCaptchaForManualLogin: (image: string | null, execution: string | null) => void;
  submitManualCaptcha: (captchaCode: string, execution: string) => Promise<void>;
  setSyncProgress: (progress: SyncProgress | null) => void;
  clearSyncProgress: () => void;
  showError: (message: string) => void;
  setShowErrorDialog: (show: boolean) => void;
  setShowP2PDialog: (show: boolean) => void;
  loadP2PStatus: () => Promise<void>;
  setP2PTransferProgress: (progress: P2PTransferProgress | null) => void;
  setPendingPairRequest: (request: P2PPairingRequest | null) => void;
  loadStatisticsSummary: (params: tauri.StatisticsParams) => Promise<void>;
  loadTodaySummary: (params: tauri.StatisticsParams) => Promise<void>;
  loadMonthSummary: (params: tauri.StatisticsParams) => Promise<void>;
  loadDailyTrend: (params: tauri.StatisticsParams) => Promise<void>;
  loadCategoryDistribution: (params: tauri.StatisticsParams) => Promise<void>;
  loadMealDistribution: (params: tauri.StatisticsParams) => Promise<void>;
  loadConsumptionDistribution: (params: tauri.StatisticsParams) => Promise<void>;
  loadMerchantRanking: (params: tauri.StatisticsParams) => Promise<void>;
  loadForgotCardStats: (params: tauri.StatisticsParams) => Promise<void>;
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
  forgotCardStats: null,
  isLoadingStatistics: false,

  isLoading: false,
  showStartupDialog: false,
  showIdentitySelectDialog: false,
  showIdentityManagerDialog: false,
  showSettingsDialog: false,
  settingsDialogTab: 'ui',
  showAboutDialog: false,
  showCaptchaTestDialog: false,
  showDataTransferDialog: false,
  showStatisticsDialog: false,
  showSyncRangeDialog: false,
  showManualCaptchaDialog: false,
  captchaImage: null,
  captchaExecution: null,
  pendingSyncAction: null,
  showErrorDialog: false,
  errorMessage: '',

  showP2PDialog: false,
  p2pStatus: null,
  p2pTransferProgress: null,
  pendingPairRequest: null,

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
      const { currentIdentity } = get();
      const nextCurrentIdentity = currentIdentity
        ? identities.find((identity) => identity.id === currentIdentity.id) ?? null
        : null;

      set({
        identities,
        currentIdentity: nextCurrentIdentity,
        ...(currentIdentity && !nextCurrentIdentity
          ? { accounts: [], bills: [], billTotal: 0 }
          : {}),
      });
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
  startSync: async (identityId, syncRange) => {
    try {
      set({
        syncProgress: {
          account_id: '',
          current_page: 0,
          total_pages: 0,
          new_items: 0,
          is_running: true,
          status: 'running',
          message: `正在执行增量同步（范围：${syncRangeLabel(syncRange)}）...`,
        },
      });
      const progress = await tauri.incremental_sync(identityId, syncRange);
      // 检查是否需要手动输入验证码
      if (progress.captcha_required && progress.captcha_image && progress.execution) {
        const previous = get().syncProgress;
        set({
          syncProgress: {
            ...(previous && previous.status === 'captcha_required' ? previous : progress),
            ...progress,
            account_id: progress.account_id || previous?.account_id || '',
            current_account: progress.current_account || previous?.current_account || '',
            account_index: progress.account_index ?? previous?.account_index,
            total_accounts: progress.total_accounts ?? previous?.total_accounts,
            status: 'captcha_required',
            message: progress.error ?? '需要输入验证码以继续同步',
          },
          captchaImage: progress.captcha_image,
          captchaExecution: progress.execution,
          showManualCaptchaDialog: true,
        });
        return;
      }
      set({
        syncProgress: {
          ...progress,
          message: progress.message ?? `增量同步完成，本次新增 ${progress.new_items} 条记录`,
        },
      });
      get().loadBills();
      // 同步完成后刷新统计数据
      get().refreshStatistics();
    } catch (e) {
      get().showError(`同步失败: ${e}`);
    }
  },

  startFullSync: async (identityId, syncRange) => {
    try {
      set({
        syncProgress: {
          account_id: '',
          current_page: 0,
          total_pages: 0,
          new_items: 0,
          is_running: true,
          status: 'running',
          message: `正在执行全量同步（范围：${syncRangeLabel(syncRange)}），可能需要一点时间...`,
        },
      });
      const progress = await tauri.full_sync(identityId, syncRange);
      if (progress.captcha_required && progress.captcha_image && progress.execution) {
        const previous = get().syncProgress;
        set({
          syncProgress: {
            ...(previous && previous.status === 'captcha_required' ? previous : progress),
            ...progress,
            account_id: progress.account_id || previous?.account_id || '',
            current_account: progress.current_account || previous?.current_account || '',
            account_index: progress.account_index ?? previous?.account_index,
            total_accounts: progress.total_accounts ?? previous?.total_accounts,
            status: 'captcha_required',
            message: progress.error ?? '需要输入验证码以继续全量同步',
          },
          captchaImage: progress.captcha_image,
          captchaExecution: progress.execution,
          showManualCaptchaDialog: true,
        });
        return;
      }
      set({
        syncProgress: {
          ...progress,
          message: progress.message ?? `全量同步完成，本次新增 ${progress.new_items} 条记录`,
        },
      });
      get().loadBills();
      get().refreshStatistics();
    } catch (e) {
      get().showError(`全量更新失败: ${e}`);
    }
  },

  startSyncAccount: async (identityId, accountId, syncRange) => {
    try {
      set({
        syncProgress: {
          account_id: accountId,
          current_page: 0,
          total_pages: 0,
          new_items: 0,
          is_running: true,
          status: 'running',
          message: `正在同步账号 ${accountId}（范围：${syncRangeLabel(syncRange)}）...`,
        },
      });
      const progress = await tauri.incremental_sync_account(identityId, accountId, syncRange);
      if (progress.captcha_required && progress.captcha_image && progress.execution) {
        const previous = get().syncProgress;
        set({
          syncProgress: {
            ...(previous && previous.status === 'captcha_required' ? previous : progress),
            ...progress,
            account_id: progress.account_id || previous?.account_id || accountId,
            current_account: progress.current_account || previous?.current_account || '',
            account_index: progress.account_index ?? previous?.account_index,
            total_accounts: progress.total_accounts ?? previous?.total_accounts,
            status: 'captcha_required',
            message: progress.error ?? `账号 ${accountId} 需要验证码`,
          },
          captchaImage: progress.captcha_image,
          captchaExecution: progress.execution,
          showManualCaptchaDialog: true,
        });
        return;
      }
      set({
        syncProgress: {
          ...progress,
          message: progress.message ?? `账号 ${accountId} 同步完成，本次新增 ${progress.new_items} 条记录`,
        },
      });
      get().loadBills();
      get().refreshStatistics();
    } catch (e) {
      get().showError(`账号增量更新失败: ${e}`);
    }
  },

  startFullSyncAccount: async (identityId, accountId, syncRange) => {
    try {
      set({
        syncProgress: {
          account_id: accountId,
          current_page: 0,
          total_pages: 0,
          new_items: 0,
          is_running: true,
          status: 'running',
          message: `正在全量同步账号 ${accountId}（范围：${syncRangeLabel(syncRange)}）...`,
        },
      });
      const progress = await tauri.full_sync_account(identityId, accountId, syncRange);
      if (progress.captcha_required && progress.captcha_image && progress.execution) {
        const previous = get().syncProgress;
        set({
          syncProgress: {
            ...(previous && previous.status === 'captcha_required' ? previous : progress),
            ...progress,
            account_id: progress.account_id || previous?.account_id || accountId,
            current_account: progress.current_account || previous?.current_account || '',
            account_index: progress.account_index ?? previous?.account_index,
            total_accounts: progress.total_accounts ?? previous?.total_accounts,
            status: 'captcha_required',
            message: progress.error ?? `账号 ${accountId} 需要验证码`,
          },
          captchaImage: progress.captcha_image,
          captchaExecution: progress.execution,
          showManualCaptchaDialog: true,
        });
        return;
      }
      set({
        syncProgress: {
          ...progress,
          message: progress.message ?? `账号 ${accountId} 全量同步完成，本次新增 ${progress.new_items} 条记录`,
        },
      });
      get().loadBills();
      get().refreshStatistics();
    } catch (e) {
      get().showError(`账号全量更新失败: ${e}`);
    }
  },

  openSyncRangeDialog: (action) => set({ pendingSyncAction: action, showSyncRangeDialog: true }),
  closeSyncRangeDialog: () => set({ showSyncRangeDialog: false, pendingSyncAction: null }),
  confirmSyncRange: async (syncRange) => {
    const action = get().pendingSyncAction;
    if (!action) return;

    set({ showSyncRangeDialog: false, pendingSyncAction: null });

    switch (action.kind) {
      case 'identity_incremental':
        await get().startSync(action.identityId, syncRange);
        return;
      case 'identity_full':
        await get().startFullSync(action.identityId, syncRange);
        return;
      case 'account_incremental':
        await get().startSyncAccount(action.identityId, action.accountId, syncRange);
        return;
      case 'account_full':
        await get().startFullSyncAccount(action.identityId, action.accountId, syncRange);
        return;
    }
  },
  setShowStartupDialog: (show) => set({ showStartupDialog: show }),
  setShowIdentitySelectDialog: (show) => set({ showIdentitySelectDialog: show }),
  setShowIdentityManagerDialog: (show) => set({ showIdentityManagerDialog: show }),
  setShowSettingsDialog: (show) =>
    set((state) => ({
      showSettingsDialog: show,
      settingsDialogTab: show ? state.settingsDialogTab : 'ui',
    })),
  openSettingsDialog: (tab = 'ui') =>
    set({
      showSettingsDialog: true,
      settingsDialogTab: tab,
    }),
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
  submitManualCaptcha: async (captchaCode, execution) => {
    const { currentIdentity } = get();
    if (!currentIdentity || !captchaCode.trim()) return;

    // Close the dialog immediately and continue syncing in the background.
    set({
      syncProgress: {
        account_id: '',
        current_page: 0,
        total_pages: 0,
        new_items: 0,
        is_running: true,
        status: 'running',
        message: '验证码已提交，正在继续同步...',
      },
      showManualCaptchaDialog: false,
      captchaImage: null,
      captchaExecution: null,
    });

    try {
      const progress = await tauri.sync_with_captcha(
        currentIdentity.id,
        captchaCode.trim(),
        execution
      );

      if (progress.captcha_required && progress.captcha_image && progress.execution) {
        const previous = get().syncProgress;
        set({
          syncProgress: {
            ...(previous && previous.status === 'captcha_required' ? previous : progress),
            ...progress,
            account_id: progress.account_id || previous?.account_id || '',
            current_account: progress.current_account || previous?.current_account || '',
            account_index: progress.account_index ?? previous?.account_index,
            total_accounts: progress.total_accounts ?? previous?.total_accounts,
            status: 'captcha_required',
            message: progress.error ?? '需要继续输入验证码',
          },
          captchaImage: progress.captcha_image,
          captchaExecution: progress.execution,
          showManualCaptchaDialog: true,
        });
        return;
      }

      set({
        syncProgress: {
          ...progress,
          message: progress.message ?? `同步完成，本次新增 ${progress.new_items} 条记录`,
        },
      });
      await get().loadBills();
      await get().refreshStatistics();
    } catch (e) {
      get().showError(`验证码提交后继续同步失败: ${e}`);
    }
  },
  setSyncProgress: (progress) => set({ syncProgress: progress }),
  clearSyncProgress: () => set({ syncProgress: null }),

  showError: (message) => {
    console.error('[App Error]', message);
    set({ showErrorDialog: true, errorMessage: message });
    // 发送错误到后端记录
    tauri.log_error(message).catch(console.error);
  },

  setShowErrorDialog: (show) => set({ showErrorDialog: show }),

  setShowP2PDialog: (show) => set({ showP2PDialog: show }),

  loadP2PStatus: async () => {
    try {
      const status = await tauri.p2p_get_status();
      set({ p2pStatus: status });
    } catch (e) {
      console.error('Failed to load P2P status:', e);
    }
  },

  setP2PTransferProgress: (progress) => set({ p2pTransferProgress: progress }),

  setPendingPairRequest: (request) => set({ pendingPairRequest: request }),

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

  loadForgotCardStats: async (params) => {
    try {
      const stats = await tauri.get_forgot_card_stats(params);
      set({ forgotCardStats: stats });
    } catch (e) {
      console.error('Failed to load forgot card stats:', e);
      set({ forgotCardStats: null });
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
