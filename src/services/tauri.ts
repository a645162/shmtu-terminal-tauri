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
  LocalOcrModelStatus,
  StatisticsSummary,
  DailyTrendItem,
  CategoryItem,
  MealDistItem,
  ReclassifyResult,
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
    model_version: 'v1' | 'v2';
    model_tag: string;
    model_backbone: string;
    model_precision: string;
  };
  sync: {
    max_pages: number;
    early_stop_threshold: number;
    skip_graduated_accounts: boolean;
    auto_merge_after_sync: boolean;
    auto_sync_enabled: boolean;
    auto_sync_interval_minutes: number;
    auto_sync_range: SyncRangePreset;
  };
  data: {
    data_directory: string;
    snapshot_keep_count: number;
  };
  p2p: {
    auto_start: boolean;
    auto_accept: boolean;
    auto_reconnect: boolean;
    device_name: string;
    port: number;
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

export interface AutoSyncStatus {
  is_running: boolean;
  last_run_seconds_ago?: number;
  next_run_in_seconds?: number;
  total_runs: number;
  success_runs: number;
  failed_runs: number;
}

export async function load_config(): Promise<AppConfig> {
  return invoke<AppConfig>('load_config');
}

export async function save_config(config: AppConfig): Promise<void> {
  return invoke('save_config', { config });
}

export async function get_auto_sync_status(): Promise<AutoSyncStatus> {
  return invoke<AutoSyncStatus>('get_auto_sync_status');
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

export async function get_bill_detail(identityId: number, billId: number): Promise<BillItem> {
  return invoke<BillItem>('get_bill_detail', { identityId, billId });
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

export async function rebuild_merged_bills(identityId: number): Promise<number> {
  return invoke<number>('rebuild_merged_bills', { identityId });
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

export interface CaptchaChallengeResponse {
  captcha_image: string;
  execution: string;
}

export async function get_captcha_image(): Promise<string> {
  return invoke<string>('get_captcha_image');
}

export async function get_captcha_with_execution(): Promise<CaptchaChallengeResponse> {
  return invoke<CaptchaChallengeResponse>('get_captcha_with_execution');
}

export async function test_captcha(mode: CaptchaMode, manualInput?: string): Promise<CaptchaTestResult> {
  return invoke<CaptchaTestResult>('test_captcha', { mode, manualInput });
}

export async function batch_test_captcha(mode: CaptchaMode, count: number): Promise<CaptchaTestResult[]> {
  return invoke<CaptchaTestResult[]>('batch_test_captcha', { mode, count });
}

export async function get_local_ocr_model_status(): Promise<LocalOcrModelStatus> {
  return invoke<LocalOcrModelStatus>('get_local_ocr_model_status');
}

export async function ensure_local_ocr_models(): Promise<LocalOcrModelStatus> {
  return invoke<LocalOcrModelStatus>('ensure_local_ocr_models');
}

export async function cancel_local_ocr_model_download(): Promise<void> {
  return invoke('cancel_local_ocr_model_download');
}

export async function delete_local_ocr_models(): Promise<LocalOcrModelStatus> {
  return invoke<LocalOcrModelStatus>('delete_local_ocr_models');
}

export async function init_local_ocr(): Promise<void> {
  return invoke('init_local_ocr');
}

export async function unload_local_ocr(): Promise<void> {
  return invoke('unload_local_ocr');
}

export async function get_ocr_model_version(): Promise<'v1' | 'v2'> {
  return invoke<'v1' | 'v2'>('get_ocr_model_version');
}

export async function set_ocr_model_version(version: 'v1' | 'v2'): Promise<'v1' | 'v2'> {
  return invoke<'v1' | 'v2'>('set_ocr_model_version', { version });
}

export interface OcrV2TagCatalogEntry {
  tag: string;
  published_at: string | null;
  prerelease: boolean;
}

export async function get_ocr_v2_tag_catalog(): Promise<OcrV2TagCatalogEntry[]> {
  return invoke<OcrV2TagCatalogEntry[]>('get_ocr_v2_tag_catalog');
}

export async function refresh_ocr_v2_tag_catalog(): Promise<OcrV2TagCatalogEntry[]> {
  return invoke<OcrV2TagCatalogEntry[]>('refresh_ocr_v2_tag_catalog');
}

export async function get_ocr_v2_model_tag(): Promise<string> {
  return invoke<string>('get_ocr_v2_model_tag');
}

export async function set_ocr_v2_model_tag(tag: string): Promise<string> {
  return invoke<string>('set_ocr_v2_model_tag', { tag });
}

export async function ocr_v2_resolve_latest_tag(): Promise<string> {
  return invoke<string>('ocr_v2_resolve_latest_tag');
}

// ---- v2 OCR Model Selection API ----

export interface OcrV2ModelInfo {
  asset_stem: string;
  display_name: string;
  backbone: string;
  model_size_m: number | null;
  val_acc_expression: number | null;
  test_acc_expression: number | null;
}

export interface OcrV2Config {
  tag: string;
  backbone: string;
  precision: string;
}

export async function list_ocr_v2_models(tag: string): Promise<OcrV2ModelInfo[]> {
  return invoke<OcrV2ModelInfo[]>('list_ocr_v2_models', { tag });
}

export async function set_ocr_v2_backbone(backbone: string): Promise<void> {
  return invoke('set_ocr_v2_backbone', { backbone });
}

export async function set_ocr_v2_precision(precision: string): Promise<void> {
  return invoke('set_ocr_v2_precision', { precision });
}

export async function get_ocr_v2_config(): Promise<OcrV2Config> {
  return invoke<OcrV2Config>('get_ocr_v2_config');
}

// ---- Local Model Scanning & Selection API ----

export interface LocalOcrModelEntry {
  file_name: string;
  file_size: number;
  version: string;
  backbone: string;
  precision: string;
}

export async function scan_local_ocr_models(): Promise<LocalOcrModelEntry[]> {
  return invoke<LocalOcrModelEntry[]>('scan_local_ocr_models');
}

export async function select_local_ocr_model(
  version: string,
  backbone: string,
  precision: string,
): Promise<string> {
  return invoke<string>('select_local_ocr_model', { version, backbone, precision });
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

export interface ForgotCardItem {
  id: number;
  date: string;
  time: string;
  amount: number;
  targetUser: string;
}

export interface ForgotCardStats {
  count: number;
  totalAmount: number;
  items: ForgotCardItem[];
}

export async function get_forgot_card_stats(params: StatisticsParams): Promise<ForgotCardStats> {
  return invoke<ForgotCardStats>('get_forgot_card_stats', { params });
}

export interface CategoryBillItem {
  id: number;
  date: string;
  time: string;
  itemType: string;
  targetUser: string;
  amount: number;
  method: string;
}

export async function get_category_bills(params: CategorySummaryParams): Promise<CategoryBillItem[]> {
  return invoke<CategoryBillItem[]>('get_category_bills', { params });
}

// ========== Classification Rules (Dynamic Loading) ==========

export async function get_classification_rules(): Promise<import('../types').ClassificationRules> {
  return invoke<import('../types').ClassificationRules>('get_classification_rules');
}

// ========== Reclassify Historical Bills ==========

export async function reclassify_all_bills(): Promise<ReclassifyResult> {
  return invoke<ReclassifyResult>('reclassify_all_bills');
}

export async function reclassify_bills_by_identity(identityId: number): Promise<ReclassifyResult> {
  return invoke<ReclassifyResult>('reclassify_bills_by_identity', { identityId });
}

// ========== Startup Protection ==========

export async function verify_startup_password(password: string): Promise<boolean> {
  return invoke<boolean>('verify_startup_password', { password });
}

export async function set_startup_password(password: string): Promise<void> {
  return invoke('set_startup_password', { password });
}

// ========== App Info ==========

export interface GitContributor {
  name: string;
  email: string;
  github_url: string;
}

export async function get_app_version(): Promise<string> {
  return invoke<string>('get_app_version');
}

export async function check_for_updates(): Promise<string | null> {
  return invoke<string | null>('check_for_updates');
}

export async function get_git_contributors(): Promise<GitContributor[]> {
  return invoke<GitContributor[]>('get_git_contributors');
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

// ========== Person Account (一卡通个人账户) ==========

export async function fetch_person_account(accountDbId: number): Promise<import('../types').PersonAccountInfo> {
  return invoke<import('../types').PersonAccountInfo>('fetch_person_account', { accountDbId });
}

export async function get_cached_person_account(accountDbId: number): Promise<import('../types').PersonAccountInfo | null> {
  return invoke<import('../types').PersonAccountInfo | null>('get_cached_person_account', { accountDbId });
}

export async function list_cached_person_accounts(accountDbIds: number[]): Promise<import('../types').PersonAccountInfo[]> {
  return invoke<import('../types').PersonAccountInfo[]>('list_cached_person_accounts', { accountDbIds });
}

export async function submit_person_account_captcha(
  accountDbId: number,
  captchaCode: string,
  execution: string
): Promise<import('../types').PersonAccountInfo> {
  return invoke<import('../types').PersonAccountInfo>('submit_person_account_captcha', {
    accountDbId,
    captchaCode,
    execution,
  });
}

// ========== Manual Captcha Sync ==========

export async function sync_with_captcha(
  identityId: number,
  captchaCode: string,
  execution: string
): Promise<SyncProgress> {
  return invoke<SyncProgress>('sync_with_captcha', { identityId, captchaCode, execution });
}

export async function log_error(message: string): Promise<void> {
  return invoke('log_error', { message });
}

// ========== Debug (清除 cookies / 缓存) ==========

export interface ClearCookiesSummary {
  accounts_visited: number;
  sessions_cleared: number;
  캐ches_cleared: number;
}

export async function clear_all_cookies(): Promise<ClearCookiesSummary> {
  return invoke<ClearCookiesSummary>('clear_all_cookies');
}

// ========== Remote Access (RESTful) ==========

export interface RemoteSessionFrontend {
  session_id: string;
  base_url: string;
  device_name: string;
  has_token: boolean;
}

export async function remote_connect(base_url: string, device_name: string): Promise<RemoteSessionFrontend> {
  return invoke<RemoteSessionFrontend>('remote_connect', { baseUrl: base_url, deviceName: device_name });
}

export async function remote_disconnect(session_id: string): Promise<void> {
  return invoke('remote_disconnect', { sessionId: session_id });
}

export async function remote_list_sessions(): Promise<RemoteSessionFrontend[]> {
  return invoke<RemoteSessionFrontend[]>('remote_list_sessions');
}

export async function remote_list_identities(session_id: string): Promise<unknown[]> {
  return invoke<unknown[]>('remote_list_identities', { sessionId: session_id });
}

export async function remote_list_bills(session_id: string, query: Record<string, string> = {}): Promise<unknown[]> {
  return invoke<unknown[]>('remote_list_bills', { sessionId: session_id, query });
}

export async function remote_export(session_id: string): Promise<string> {
  return invoke<string>('remote_export', { sessionId: session_id });
}

// ========== 云备份 ==========
export async function cloud_backup_get_config() { return invoke('cloud_backup_get_config') as Promise<import('../types').WebDavConfig>; }
export async function cloud_backup_save_config(config: import('../types').WebDavConfig) { return invoke<void>('cloud_backup_save_config', { config }); }
export async function cloud_backup_test_connection() { return invoke<boolean>('cloud_backup_test_connection'); }
export async function cloud_backup_test_write_read() { return invoke<string>('cloud_backup_test_write_read'); }
export async function cloud_backup_now(password?: string) { return invoke<string>('cloud_backup_now', { password: password || null }); }
export async function cloud_backup_restore(remote_path: string, password?: string) { return invoke('cloud_backup_restore', { remotePath: remote_path, password: password || null }) as Promise<import('../types').RestoreReport>; }
export async function cloud_backup_list_remote() { return invoke('cloud_backup_list_remote') as Promise<import('../types').BackupMeta[]>; }
export async function cloud_backup_delete_remote(remote_path: string) { return invoke<boolean>('cloud_backup_delete_remote', { remotePath: remote_path }); }
export async function cloud_backup_get_auto_config() { return invoke('cloud_backup_get_auto_config') as Promise<import('../types').CloudBackupAutoConfig>; }
export async function cloud_backup_set_auto_enabled(enabled: boolean) { return invoke<void>('cloud_backup_set_auto_enabled', { enabled }); }
export async function cloud_backup_set_auto_interval(minutes: number) { return invoke<void>('cloud_backup_set_auto_interval', { minutes }); }
export async function cloud_backup_set_max_keep(count: number) { return invoke<void>('cloud_backup_set_max_keep', { count }); }

// ========== P2P ==========
export async function p2p_get_status() { return invoke('p2p_get_status') as Promise<import('../types').P2PServerStatus>; }
export async function p2p_start_server(port: number) { return invoke<void>('p2p_start_server', { port }); }
export async function p2p_stop_server() { return invoke<void>('p2p_stop_server'); }
export async function p2p_set_pair_code(code: string) { return invoke<void>('p2p_set_pair_code', { code }); }
export async function p2p_discover(base_url: string, device_name: string) { return invoke('p2p_discover', { baseUrl: base_url, deviceName: device_name }) as Promise<import('../types').P2PRestDiscoverData>; }
export async function p2p_pair(base_url: string, pair_code: string, device_name: string, listen_port: number, listen_ips: string[]) { return invoke('p2p_pair', { baseUrl: base_url, pairCode: pair_code, deviceName: device_name, listenPort: listen_port, listenIps: listen_ips }) as Promise<import('../types').P2PRestPairResponseData>; }
export async function p2p_upload_transfer(base_url: string, peer_key: string, session_id: string, bill_count: number, zip_data: number[]) { return invoke('p2p_upload_transfer', { baseUrl: base_url, peerKey: peer_key, sessionId: session_id, billCount: bill_count, zipData: zip_data }) as Promise<import('../types').P2PRestTransferResponseData>; }
export async function p2p_download_transfer(base_url: string, peer_key: string, session_id: string) { return invoke('p2p_download_transfer', { baseUrl: base_url, peerKey: peer_key, sessionId: session_id }) as Promise<number[]>; }
