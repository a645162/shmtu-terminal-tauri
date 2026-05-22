import { create } from 'zustand';
import type {
  Identity,
  Account,
  BillItem,
  BillType,
  SyncProgress,
  AppTheme,
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

  isLoading: false,
  showStartupDialog: false,
  showIdentitySelectDialog: false,
  showIdentityManagerDialog: false,
  showSettingsDialog: false,
  showAboutDialog: false,
  showCaptchaTestDialog: false,
  showDataTransferDialog: false,
  showStatisticsDialog: false,

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

  loadBills: async () => {
    const { currentIdentity, billPage, billPageSize, billType, billKeyword, billDateStart, billDateEnd } = get();
    if (!currentIdentity) return;
    set({ isLoading: true });
    try {
      const result = await tauri.query_bills({
        identityId: currentIdentity.id,
        billType,
        page: billPage,
        pageSize: billPageSize,
        keyword: billKeyword || undefined,
        dateStart: billDateStart || undefined,
        dateEnd: billDateEnd || undefined,
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
    // Persist theme change
    tauri.save_config({ ui: { theme, language: 'zh-CN' } }).catch(console.error);
  },

  startSync: async (identityId) => {
    try {
      const progress = await tauri.incremental_sync(identityId);
      set({ syncProgress: progress });
      // Refresh bills after sync
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
}));
