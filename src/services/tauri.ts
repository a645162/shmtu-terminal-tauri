import { invoke } from '@tauri-apps/api/core';
import type {
  Identity,
  Account,
  BillItem,
  BillType,
  CaptchaMode,
  AppTheme,
  SyncProgress,
  SnapshotInfo,
  ExportFormat,
  CaptchaTestResult,
  StatisticsSummary,
  DailyTrendItem,
  CategoryItem,
  MealDistItem,
} from '../types';

// ========== Config ==========

export interface AppConfig {
  security: {
    enable_startup_protection: boolean;
    password_hash: string;
  };
  identity: {
    remember_default: boolean;
    default_identity_id: number;
  };
  captcha: {
    mode: CaptchaMode;
    remote_ocr_host: string;
    remote_ocr_port: number;
    onnx_model_path: string;
    ocr_retry_count: number;
  };
  sync: {
    max_pages: number;
    early_stop_threshold: number;
    auto_merge_after_sync: boolean;
  };
  data: {
    data_directory: string;
    snapshot_keep_count: number;
  };
  classification: {
    rules_path: string;
    rules_update_url: string;
  };
  update: {
    auto_check: boolean;
    check_interval_hours: number;
    last_check_time: string;
  };
  ui: {
    theme: AppTheme;
    language: string;
  };
}

export async function load_config(): Promise<AppConfig> {
  return invoke<AppConfig>('load_config');
}

export async function save_config(config: Partial<AppConfig>): Promise<void> {
  return invoke('save_config', { config });
}

// ========== Identity ==========

export async function list_identities(): Promise<Identity[]> {
  return invoke<Identity[]>('list_identities');
}

export async function create_identity(name: string): Promise<Identity> {
  return invoke<Identity>('create_identity', { name });
}

export async function update_identity(identity: Partial<Identity> & { id: number }): Promise<void> {
  return invoke('update_identity', { identity });
}

export async function delete_identity(id: number): Promise<void> {
  return invoke('delete_identity', { id });
}

// ========== Account ==========

export async function list_accounts(identityId: number): Promise<Account[]> {
  return invoke<Account[]>('list_accounts', { identityId });
}

export async function create_account(account: Omit<Account, 'id' | 'created_at' | 'updated_at'>): Promise<Account> {
  return invoke<Account>('create_account', { account });
}

export async function update_account(account: Partial<Account> & { id: number }): Promise<void> {
  return invoke('update_account', { account });
}

export async function delete_account(id: number): Promise<void> {
  return invoke('delete_account', { id });
}

// ========== Bill ==========

export interface BillQueryParams {
  identityId?: number;
  accountId?: string;
  billType: BillType;
  page: number;
  pageSize: number;
  keyword?: string;
  dateStart?: string;
  dateEnd?: string;
}

export interface BillQueryResult {
  items: BillItem[];
  total: number;
  page: number;
  page_size: number;
}

export async function query_bills(params: BillQueryParams): Promise<BillQueryResult> {
  return invoke<BillQueryResult>('query_bills', { params });
}

export async function delete_merged_bill(identityId: number, billId: number): Promise<void> {
  return invoke('delete_merged_bill', { identityId, billId });
}

// ========== Sync ==========

export async function incremental_sync(identityId: number): Promise<SyncProgress> {
  return invoke<SyncProgress>('incremental_sync', { identityId });
}

export async function full_sync(identityId: number): Promise<SyncProgress> {
  return invoke<SyncProgress>('full_sync', { identityId });
}

export async function get_sync_progress(): Promise<SyncProgress> {
  return invoke<SyncProgress>('get_sync_progress');
}

// ========== Auth ==========

export async function cas_login(accountId: string, password: string, captchaCode: string): Promise<boolean> {
  return invoke<boolean>('cas_login', { accountId, password, captchaCode });
}

export async function check_login_status(accountId: string): Promise<boolean> {
  return invoke<boolean>('check_login_status', { accountId });
}

// ========== Captcha ==========

export async function get_captcha_image(): Promise<string> {
  return invoke<string>('get_captcha_image');
}

export async function test_captcha(mode: CaptchaMode): Promise<CaptchaTestResult> {
  return invoke<CaptchaTestResult>('test_captcha', { mode });
}

export async function batch_test_captcha(mode: CaptchaMode, count: number): Promise<CaptchaTestResult[]> {
  return invoke<CaptchaTestResult[]>('batch_test_captcha', { mode, count });
}

// ========== Data Transfer ==========

export interface ExportParams {
  identityId: number;
  format: ExportFormat;
  sourceType: 'original' | 'merged';
  filePath: string;
  dateStart?: string;
  dateEnd?: string;
}

export async function export_data(params: ExportParams): Promise<string> {
  return invoke<string>('export_data', { params });
}

export async function import_data(filePath: string, identityId: number): Promise<number> {
  return invoke<number>('import_data', { filePath, identityId });
}

// ========== Snapshot ==========

export async function list_snapshots(): Promise<SnapshotInfo[]> {
  return invoke<SnapshotInfo[]>('list_snapshots');
}

export async function create_snapshot(): Promise<SnapshotInfo> {
  return invoke<SnapshotInfo>('create_snapshot');
}

export async function restore_snapshot(filename: string): Promise<void> {
  return invoke('restore_snapshot', { filename });
}

// ========== Statistics ==========

export interface StatisticsParams {
  identityId: number;
  dateStart?: string;
  dateEnd?: string;
}

export async function get_statistics_summary(params: StatisticsParams): Promise<StatisticsSummary> {
  return invoke<StatisticsSummary>('get_statistics_summary', { params });
}

export async function get_daily_trend(params: StatisticsParams): Promise<DailyTrendItem[]> {
  return invoke<DailyTrendItem[]>('get_daily_trend', { params });
}

export async function get_category_distribution(params: StatisticsParams): Promise<CategoryItem[]> {
  return invoke<CategoryItem[]>('get_category_distribution', { params });
}

export async function get_meal_distribution(params: StatisticsParams): Promise<MealDistItem[]> {
  return invoke<MealDistItem[]>('get_meal_distribution', { params });
}

// ========== Startup Protection ==========

export async function verify_startup_password(password: string): Promise<boolean> {
  return invoke<boolean>('verify_startup_password', { password });
}

export async function set_startup_password(password: string): Promise<void> {
  return invoke('set_startup_password', { password });
}

// ========== App Info ==========

export async function get_app_version(): Promise<string> {
  return invoke<string>('get_app_version');
}

export async function check_for_updates(): Promise<string | null> {
  return invoke<string | null>('check_for_updates');
}
