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
    last_identity_id: number;
  };
  captcha: {
    mode: CaptchaMode;
    remote_ocr_host: string;
    remote_ocr_port: number;
    remote_ocr_http_url: string;
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
    decimal_places: number;
    home_trend_range: string;
    home_category_range: string;
  };
}

export async function load_config(): Promise<AppConfig> {
  return invoke<AppConfig>('load_config');
}

export async function save_config(config: AppConfig): Promise<void> {
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

export interface DedupeResult {
  backfilled_count: number;
  removed_count: number;
}

export type SyncRangePreset = 'week' | 'half_month' | 'month' | 'half_year' | 'year' | 'all';

export async function query_bills(params: BillQueryParams): Promise<BillQueryResult> {
  return invoke<BillQueryResult>('query_bills', { params });
}

export async function delete_merged_bill(identityId: number, billId: number): Promise<void> {
  return invoke('delete_merged_bill', { identityId, billId });
}

export async function update_bill_notes(identityId: number, billId: number, notes: string | null): Promise<void> {
  return invoke('update_bill_notes', { identityId, billId, notes });
}

export async function dedupe_identity_bills(identityId: number): Promise<DedupeResult> {
  return invoke<DedupeResult>('dedupe_identity_bills', { identityId });
}

export async function dedupe_account_bills(identityId: number, accountId: string): Promise<DedupeResult> {
  return invoke<DedupeResult>('dedupe_account_bills', { identityId, accountId });
}

// ========== Sync ==========

export async function incremental_sync(identityId: number, syncRange: SyncRangePreset): Promise<SyncProgress> {
  return invoke<SyncProgress>('incremental_sync', { identityId, syncRange });
}

export async function full_sync(identityId: number, syncRange: SyncRangePreset): Promise<SyncProgress> {
  return invoke<SyncProgress>('full_sync', { identityId, syncRange });
}

export async function incremental_sync_account(identityId: number, accountId: string, syncRange: SyncRangePreset): Promise<SyncProgress> {
  return invoke<SyncProgress>('incremental_sync_account', { identityId, accountId, syncRange });
}

export async function full_sync_account(identityId: number, accountId: string, syncRange: SyncRangePreset): Promise<SyncProgress> {
  return invoke<SyncProgress>('full_sync_account', { identityId, accountId, syncRange });
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

export async function test_captcha(mode: CaptchaMode, manualInput?: string): Promise<CaptchaTestResult> {
  return invoke<CaptchaTestResult>('test_captcha', { mode, manualInput });
}

export async function batch_test_captcha(mode: CaptchaMode, count: number): Promise<CaptchaTestResult[]> {
  return invoke<CaptchaTestResult[]>('batch_test_captcha', { mode, count });
}

export async function init_local_ocr(): Promise<void> {
  return invoke('init_local_ocr');
}

export async function unload_local_ocr(): Promise<void> {
  return invoke('unload_local_ocr');
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

export async function get_consumption_distribution(params: StatisticsParams): Promise<import('../types').ConsumptionBucketItem[]> {
  return invoke<import('../types').ConsumptionBucketItem[]>('get_consumption_distribution', { params });
}

export async function get_merchant_ranking(params: StatisticsParams): Promise<import('../types').MerchantRankingItem[]> {
  return invoke<import('../types').MerchantRankingItem[]>('get_merchant_ranking', { params });
}

// ========== Category Summary ==========

export interface CategorySummaryParams {
  identityId: number;
  category: string;
  dateStart?: string;
  dateEnd?: string;
}

export interface CategorySummary {
  category: string;
  total_amount: number;
  count: number;
  daily_average: number;
  avg_per_transaction: number;
}

export async function get_category_summary(params: CategorySummaryParams): Promise<CategorySummary> {
  return invoke<CategorySummary>('get_category_summary', { params });
}

// ========== Classification Rules (Dynamic Loading) ==========

export async function get_classification_rules(): Promise<import('../types').ClassificationRules> {
  return invoke<import('../types').ClassificationRules>('get_classification_rules');
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

// ========== Default Identity ==========

export async function set_default_identity(identityId: number): Promise<void> {
  return invoke('set_default_identity', { identityId });
}

export async function get_default_identity(): Promise<number | null> {
  return invoke<number | null>('get_default_identity');
}

export async function set_last_identity(identityId: number): Promise<void> {
  return invoke('set_last_identity', { identityId });
}

export async function get_last_identity(): Promise<number | null> {
  return invoke<number | null>('get_last_identity');
}

// ========== Card Balance ==========

export async function get_card_balance(identityId: number): Promise<import('../types').CardBalance> {
  return invoke<import('../types').CardBalance>('get_card_balance', { identityId });
}

// ========== Error Logging ==========

export async function log_error(message: string): Promise<void> {
  return invoke('log_error', { message });
}

// ========== Manual Captcha Sync ==========

export async function sync_with_captcha(
  identityId: number,
  captchaCode: string,
  execution: string
): Promise<SyncProgress> {
  return invoke<SyncProgress>('sync_with_captcha', { identityId, captchaCode, execution });
}
