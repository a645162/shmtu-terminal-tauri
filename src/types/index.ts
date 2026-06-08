// Identity - represents a person (e.g., self, family member)
export interface Identity {
  id: number;
  name: string;
  enable: boolean;
  enable_update: boolean;
  birthday: string | null;
  default_remember: boolean;
  created_at: string;
  updated_at: string;
}

// Account - corresponds to a student ID / campus card
export interface Account {
  id: number;
  identity_id: number;
  account_name: string;
  account_id: string; // 12-digit student ID
  password: string;   // encrypted
  enable: boolean;
  enable_update: boolean;
  admission_date: string | null;
  graduation_date: string | null;
  expire_date: string;
  last_update_time: string;
  created_at: string;
  updated_at: string;
}

// BillOriginal - read-only bill from API
export interface BillItem {
  id: number;
  date_str: string;
  time_str: string;
  time_str_formatted: string;
  date_time_formatted: string;
  end_date_time_formatted: string | null;
  timestamp: number;
  end_timestamp: number | null;
  item_type: string;
  number: string;
  number_list: string;
  target_user: string;
  money_str: string;
  money: number;
  method: string;
  status_str: string;
  is_combined: boolean;
  account_id?: string;
  synced_at?: string;
  // Merged-specific fields
  source_account_id?: string;
  is_manual?: boolean;
  position?: string;
  room?: string;
  category?: string;
  notes?: string;
}

// OperationLog - manual operation records on merged data
export interface OperationLog {
  id: number;
  operation_type: 'add' | 'delete' | 'merge';
  record_numbers: string;
  operation_time: string;
  description: string;
  account_id?: string;
}

// SessionInfo - login session cookies
export interface SessionInfo {
  id: number;
  account_id: string;
  cookies: string;
  login_time: string;
  expire_time: string;
  is_valid: boolean;
}

// Bill type filter
export type BillType = 'all' | 'success' | 'not_paid' | 'failure';

// Bill status
export type BillItemStatus = 'all' | 'wait_for' | 'success' | 'failure';

// Captcha mode
export type CaptchaMode = 'manual' | 'remote_ocr' | 'remote_ocr_http' | 'local_onnx';

// Captcha answer kind
export type CaptchaAnswerKind = 'answer' | 'expression';
export type SyncRangePreset = 'week' | 'half_month' | 'month' | 'half_year' | 'year' | 'all';

// App theme
export type AppTheme = 'light' | 'dark' | 'system';
export type AppSettingsTab =
  | 'security'
  | 'identity'
  | 'captcha'
  | 'sync'
  | 'data'
  | 'p2p'
  | 'ui'
  | 'home'
  | 'classification'
  | 'update'
  | 'debug';

// App UI config
export interface AppUiConfig {
  theme: AppTheme;
  language: string;
  decimal_places: number;
}

// Captcha test result
export interface CaptchaTestResult {
  id: number;
  success: boolean;
  expression: string;
  answer: string;
  duration_ms: number;
  mode: CaptchaMode;
  verification?: string;
  error?: string;
  captcha_required?: boolean;
  captcha_image?: string;
  execution?: string;
}

export interface LocalOcrModelStatus {
  model_dir: string;
  ready: boolean;
  total_files: number;
  existing_files: number;
  missing_files: string[];
}

export interface LocalOcrModelDownloadProgress {
  phase: 'checking' | 'downloading' | 'completed' | 'cancelled' | 'error';
  model_dir: string;
  total_files: number;
  completed_files: number;
  current_file_index?: number;
  current_file_name?: string;
  current_file_progress: number;
  overall_progress: number;
  downloaded_bytes?: number;
  total_bytes?: number;
  message: string;
}

// Sync progress
export interface SyncProgress {
  account_id: string;
  current_account?: string;
  account_index?: number;
  total_accounts?: number;
  current_page: number;
  total_pages: number;
  new_items: number;
  is_running: boolean;
  status: 'idle' | 'running' | 'completed' | 'error' | 'captcha_required';
  error?: string;
  message?: string;
  captcha_required?: boolean;
  captcha_image?: string;
  execution?: string;
}

// Snapshot info
export interface SnapshotInfo {
  filename: string;
  created_at: string;
  size_bytes: number;
}

// Export format
export type ExportFormat = 'csv' | 'json' | 'qianji';

// Statistics summary
export interface StatisticsSummary {
  total_expense: number;
  total_income: number;
  net_expense: number;
  daily_average: number;
  expense_count: number;
  income_count: number;
}

// Classification result
export interface ClassificationResult {
  type: string;
  building: string | null;
  room: string | null;
  meal: string | null;
}

// Daily trend data
export interface DailyTrendItem {
  date: string;
  expense: number;
  income: number;
}

// Category distribution item
export interface CategoryItem {
  name: string;
  value: number;
  count: number;
  color: string;
}

// Meal distribution item
export interface MealDistItem {
  name: string;
  count: number;
  amount: number;
}

// Consumption bucket item (histogram data)
export interface ConsumptionBucketItem {
  range: string;
  count: number;
  amount: number;
}

// Merchant ranking item
export interface MerchantRankingItem {
  merchant: string;
  count: number;
  amount: number;
}

// Card balance
export interface CardBalance {
  account_id: string;
  balance: number;
  last_updated: string;
}

// 一卡通个人账户详情（CAS /epay/personaccount/index）
export interface PersonAccountInfo {
  account_id: string;
  real_name: string;
  real_name_auth_status: string;
  cash_balance: number;
  cash_balance_raw: string;
  security_question_status: string;
  register_date: string;
  student_id: string;
  email: string;
  nickname: string;
  gender: string;
  class_name: string;
  phone_num: string;
  gender_from_id: string;
  id_type: string;
  id_number: string;
  remark: string;
  user_type: string;
  csrf_token: string;
  csrf_header: string;
  fetched_at: string;
}

// Classification rules (loaded from backend / database/bill/)
// 数据来源：rules.toml → Rust TOML 解析 → JSON → 前端
export interface ClassificationRules {
  type?: Record<string, {
    match_field: string;
    match_names: string[];
    match_targets: string[];
  }>;
  type_rules: Record<string, {
    match_field: string;
    match_names: string[];
    match_targets: string[];
  }>;
  position: {
    field: string;
    keywords: Record<string, { building: string; room: string }>;
  };
  schedule: Array<{
    valid_date: { start_date: string; end_date: string };
    timetable: Record<string, { name: string; start_time: string; end_time: string }>;
  }>;
}

// 重算历史账单的结果
export interface ReclassifyResult {
  total_scanned: number;
  translated: number;
  category_updated: number;
  missed: number;
  duration_ms: number;
}
