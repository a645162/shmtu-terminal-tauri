use serde::{Deserialize, Serialize};

/// 身份信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identity {
    pub id: i64,
    pub name: String,
    pub enable: bool,
    pub enable_update: bool,
    pub birthday: Option<String>,
    pub default_remember: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// 创建身份的参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateIdentityParams {
    pub name: String,
    pub enable: Option<bool>,
    pub enable_update: Option<bool>,
    pub birthday: Option<String>,
    pub default_remember: Option<bool>,
}

/// 账号信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: i64,
    pub identity_id: i64,
    pub account_name: String,
    pub account_id: String,
    pub password: String,
    pub enable: bool,
    pub enable_update: bool,
    pub admission_date: Option<String>,
    pub graduation_date: Option<String>,
    pub expire_date: String,
    pub last_update_time: String,
    pub created_at: String,
    pub updated_at: String,
}

/// 创建账号的参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAccountParams {
    pub identity_id: i64,
    pub account_name: String,
    pub account_id: String,
    pub password: String,
    pub enable: Option<bool>,
    pub enable_update: Option<bool>,
    pub admission_date: Option<String>,
    pub graduation_date: Option<String>,
    pub expire_date: Option<String>,
}

/// 原始账单记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillOriginal {
    pub id: i64,
    pub date_str: String,
    pub time_str: String,
    pub time_str_formatted: Option<String>,
    pub date_time_formatted: Option<String>,
    pub end_date_time_formatted: Option<String>,
    pub timestamp: Option<i64>,
    pub end_timestamp: Option<i64>,
    pub item_type: Option<String>,
    pub number: Option<String>,
    pub number_list: Option<String>,
    pub target_user: Option<String>,
    pub money_str: Option<String>,
    pub money: Option<f64>,
    pub method: Option<String>,
    pub status_str: Option<String>,
    pub is_combined: bool,
    pub account_id: String,
    pub synced_at: Option<String>,
}

/// 合并账单记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillMerged {
    pub id: i64,
    pub date_str: String,
    pub time_str: String,
    pub time_str_formatted: Option<String>,
    pub date_time_formatted: Option<String>,
    pub end_date_time_formatted: Option<String>,
    pub timestamp: Option<i64>,
    pub end_timestamp: Option<i64>,
    pub item_type: Option<String>,
    pub number: Option<String>,
    pub number_list: Option<String>,
    pub target_user: Option<String>,
    pub money_str: Option<String>,
    pub money: Option<f64>,
    pub method: Option<String>,
    pub status_str: Option<String>,
    pub is_combined: bool,
    pub source_account_id: Option<String>,
    pub is_manual: bool,
    pub position: Option<String>,
    pub room: Option<String>,
    pub category: Option<String>,
    pub notes: Option<String>,
    pub synced_at: Option<String>,
}

/// 操作记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationLog {
    pub id: i64,
    pub operation_type: String,
    pub record_numbers: Option<String>,
    pub operation_time: String,
    pub description: Option<String>,
    pub account_id: Option<String>,
}

/// 操作类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperationType {
    Add,
    Delete,
    Merge,
}

impl OperationType {
    pub fn as_str(&self) -> &'static str {
        match self {
            OperationType::Add => "add",
            OperationType::Delete => "delete",
            OperationType::Merge => "merge",
        }
    }

    pub fn parse_from_str(s: &str) -> Option<Self> {
        match s {
            "add" => Some(OperationType::Add),
            "delete" => Some(OperationType::Delete),
            "merge" => Some(OperationType::Merge),
            _ => None,
        }
    }
}

/// 会话信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: i64,
    pub account_id: String,
    pub cookies: String,
    pub login_time: Option<String>,
    pub expire_time: Option<String>,
    pub is_valid: bool,
}
