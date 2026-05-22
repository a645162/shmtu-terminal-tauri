use serde::{Deserialize, Serialize};
use tauri::State;

use crate::db::BillStoreImpl;
use crate::state::AppState;

/// 前端统计查询参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatisticsParams {
    pub identity_id: i64,
    pub date_start: Option<String>,
    pub date_end: Option<String>,
}

/// 统计摘要（与前端 StatisticsSummary 对齐）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatisticsSummary {
    pub total_expense: f64,
    pub total_income: f64,
    pub net_expense: f64,
    pub daily_average: f64,
    pub expense_count: u32,
    pub income_count: u32,
}

/// 日消费趋势条目（与前端 DailyTrendItem 对齐）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyTrendItem {
    pub date: String,
    pub expense: f64,
    pub income: f64,
}

/// 分类分布条目（与前端 CategoryItem 对齐）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryItem {
    pub name: String,
    pub value: f64,
    pub count: u32,
    pub color: String,
}

/// 用餐时段分布条目（与前端 MealDistItem 对齐）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MealDistItem {
    pub name: String,
    pub count: u32,
    pub amount: f64,
}

/// 预定义颜色列表
const CATEGORY_COLORS: &[&str] = &[
    "#5B8FF9", "#5AD8A6", "#F6BD16", "#E86452",
    "#6DC8EC", "#945FB9", "#FF9845", "#1E9493",
    "#FF99C3", "#269A99",
];

/// 时间范围过滤辅助函数
fn date_in_range(date_str: &str, start: &Option<String>, end: &Option<String>) -> bool {
    if let Some(ref s) = start {
        if date_str < s.as_str() {
            return false;
        }
    }
    if let Some(ref e) = end {
        if date_str > e.as_str() {
            return false;
        }
    }
    true
}

#[tauri::command]
pub async fn get_statistics_summary(
    state: State<'_, AppState>,
    params: StatisticsParams,
) -> Result<StatisticsSummary, String> {
    let db = state.db_manager.read().await;
    let store = BillStoreImpl::new(db.clone_ref(), "", params.identity_id).map_err(|e| e.to_string())?;
    let bills = store.get_all_merged_bills(params.identity_id).map_err(|e| e.to_string())?;

    let mut total_expense = 0.0_f64;
    let mut total_income = 0.0_f64;
    let mut expense_count = 0u32;
    let mut income_count = 0u32;
    let mut date_set = std::collections::HashSet::new();

    for bill in &bills {
        // 时间范围过滤
        if !date_in_range(&bill.date_str, &params.date_start, &params.date_end) {
            continue;
        }

        let money = bill.money.unwrap_or(0.0);
        if money < 0.0 {
            total_expense += money.abs();
            expense_count += 1;
        } else if money > 0.0 {
            total_income += money;
            income_count += 1;
        }

        date_set.insert(bill.date_str.clone());
    }

    let days = date_set.len().max(1) as f64;

    Ok(StatisticsSummary {
        total_expense,
        total_income,
        net_expense: total_expense - total_income,
        daily_average: total_expense / days,
        expense_count,
        income_count,
    })
}

#[tauri::command]
pub async fn get_daily_trend(
    state: State<'_, AppState>,
    params: StatisticsParams,
) -> Result<Vec<DailyTrendItem>, String> {
    let db = state.db_manager.read().await;
    let store = BillStoreImpl::new(db.clone_ref(), "", params.identity_id).map_err(|e| e.to_string())?;
    let bills = store.get_all_merged_bills(params.identity_id).map_err(|e| e.to_string())?;

    let mut daily_map: std::collections::HashMap<String, (f64, f64)> = std::collections::HashMap::new();

    for bill in &bills {
        if !date_in_range(&bill.date_str, &params.date_start, &params.date_end) {
            continue;
        }

        let money = bill.money.unwrap_or(0.0);
        let entry = daily_map.entry(bill.date_str.clone()).or_insert((0.0, 0.0));
        if money < 0.0 {
            entry.0 += money.abs();
        } else {
            entry.1 += money;
        }
    }

    let mut trend: Vec<DailyTrendItem> = daily_map
        .into_iter()
        .map(|(date, (expense, income))| DailyTrendItem {
            date,
            expense,
            income,
        })
        .collect();

    trend.sort_by(|a, b| a.date.cmp(&b.date));
    Ok(trend)
}

#[tauri::command]
pub async fn get_category_distribution(
    state: State<'_, AppState>,
    params: StatisticsParams,
) -> Result<Vec<CategoryItem>, String> {
    let db = state.db_manager.read().await;
    let store = BillStoreImpl::new(db.clone_ref(), "", params.identity_id).map_err(|e| e.to_string())?;
    let bills = store.get_all_merged_bills(params.identity_id).map_err(|e| e.to_string())?;

    let classifier = state.classifier.read().await;

    let mut category_map: std::collections::HashMap<String, (f64, u32)> = std::collections::HashMap::new();

    for bill in &bills {
        if !date_in_range(&bill.date_str, &params.date_start, &params.date_end) {
            continue;
        }

        let money = bill.money.unwrap_or(0.0);
        if money >= 0.0 {
            continue; // 只统计支出
        }

        let category = if let Some(ref classifier) = *classifier {
            classifier
                .classify(
                    bill.item_type.as_deref().unwrap_or(""),
                    bill.target_user.as_deref().unwrap_or(""),
                    bill.timestamp.unwrap_or(0),
                )
                .type_label
                .clone()
                .unwrap_or_else(|| "其他".to_string())
        } else {
            "其他".to_string()
        };

        let entry = category_map.entry(category).or_insert((0.0, 0));
        entry.0 += money.abs();
        entry.1 += 1;
    }

    let mut items: Vec<CategoryItem> = category_map
        .into_iter()
        .enumerate()
        .map(|(i, (name, (value, count)))| CategoryItem {
            name,
            value,
            count,
            color: CATEGORY_COLORS[i % CATEGORY_COLORS.len()].to_string(),
        })
        .collect();

    items.sort_by(|a, b| b.value.partial_cmp(&a.value).unwrap_or(std::cmp::Ordering::Equal));
    Ok(items)
}

#[tauri::command]
pub async fn get_meal_distribution(
    state: State<'_, AppState>,
    params: StatisticsParams,
) -> Result<Vec<MealDistItem>, String> {
    let db = state.db_manager.read().await;
    let store = BillStoreImpl::new(db.clone_ref(), "", params.identity_id).map_err(|e| e.to_string())?;
    let bills = store.get_all_merged_bills(params.identity_id).map_err(|e| e.to_string())?;

    let classifier = state.classifier.read().await;

    let mut meal_map: std::collections::HashMap<String, (u32, f64)> = std::collections::HashMap::new();

    for bill in &bills {
        if !date_in_range(&bill.date_str, &params.date_start, &params.date_end) {
            continue;
        }

        let money = bill.money.unwrap_or(0.0);
        if money >= 0.0 {
            continue;
        }

        let meal = if let Some(ref classifier) = *classifier {
            classifier
                .classify(
                    bill.item_type.as_deref().unwrap_or(""),
                    bill.target_user.as_deref().unwrap_or(""),
                    bill.timestamp.unwrap_or(0),
                )
                .meal
                .clone()
                .unwrap_or_else(|| "非用餐时段".to_string())
        } else {
            "非用餐时段".to_string()
        };

        let entry = meal_map.entry(meal).or_insert((0, 0.0));
        entry.0 += 1;
        entry.1 += money.abs();
    }

    let mut items: Vec<MealDistItem> = meal_map
        .into_iter()
        .map(|(name, (count, amount))| MealDistItem {
            name,
            count,
            amount,
        })
        .collect();

    items.sort_by(|a, b| b.amount.partial_cmp(&a.amount).unwrap_or(std::cmp::Ordering::Equal));
    Ok(items)
}