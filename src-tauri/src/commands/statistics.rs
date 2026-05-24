use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::AppState;

/// 前端统计查询参数
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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

/// 消费金额分布条目（直方图数据）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsumptionBucketItem {
    pub range: String,
    pub count: u32,
    pub amount: f64,
}

/// 商户消费排行条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerchantRankingItem {
    pub merchant: String,
    pub count: u32,
    pub amount: f64,
}

/// 预定义颜色列表
const CATEGORY_COLORS: &[&str] = &[
    "#5B8FF9", "#5AD8A6", "#F6BD16", "#E86452", "#6DC8EC", "#945FB9", "#FF9845", "#1E9493",
    "#FF99C3", "#269A99",
];

/// 构建带日期范围过滤的 SQL WHERE 子句（使用参数化查询防注入）
fn build_date_filter(
    date_start: &Option<String>,
    date_end: &Option<String>,
) -> (String, Vec<String>) {
    let mut conditions = Vec::new();
    let mut params = Vec::new();

    if let Some(ref start) = date_start {
        conditions.push("date_str >= ?".to_string());
        params.push(start.clone());
    }
    if let Some(ref end) = date_end {
        conditions.push("date_str <= ?".to_string());
        params.push(end.clone());
    }

    if conditions.is_empty() {
        (String::new(), params)
    } else {
        (format!(" WHERE {}", conditions.join(" AND ")), params)
    }
}

#[tauri::command]
pub async fn get_statistics_summary(
    state: State<'_, AppState>,
    params: StatisticsParams,
) -> Result<StatisticsSummary, String> {
    let db = state.db_manager.read().await;

    let conn = db
        .open_identity_db(params.identity_id)
        .map_err(|e| e.to_string())?;

    let (date_filter, filter_params) = build_date_filter(&params.date_start, &params.date_end);

    // 查询支出总额和支出笔数（所有交易成功记录都视为支出）
    let (expense, expense_count) = {
        let sql = format!(
            "SELECT COALESCE(SUM(ABS(money)), 0), COUNT(*) FROM bill_merged{} AND status_str = '交易成功'",
            date_filter
        );
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        stmt.query_row(rusqlite::params_from_iter(filter_params.iter()), |row| {
            Ok((row.get::<_, f64>(0)?, row.get::<_, u32>(1)?))
        })
        .unwrap_or((0.0, 0))
    };

    // 收入暂时设为0（如果后续需要区分充值退款等，再扩展）
    let (income, income_count) = (0.0, 0);

    // 计算天数
    let days = if params.date_start.is_some() || params.date_end.is_some() {
        let sql = format!(
            "SELECT COUNT(DISTINCT date_str) FROM bill_merged{}",
            date_filter
        );
        conn.query_row(
            &sql,
            rusqlite::params_from_iter(filter_params.iter()),
            |row| row.get::<_, u32>(0),
        )
        .unwrap_or(1) as f64
    } else {
        let sql = "SELECT COUNT(DISTINCT date_str) FROM bill_merged";
        conn.query_row(sql, [], |row| row.get::<_, u32>(0))
            .unwrap_or(1) as f64
    };

    let days = days.max(1.0);

    Ok(StatisticsSummary {
        total_expense: expense,
        total_income: income,
        net_expense: expense - income,
        daily_average: expense / days,
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
    let conn = db
        .open_identity_db(params.identity_id)
        .map_err(|e| e.to_string())?;

    let (date_filter, filter_params) = build_date_filter(&params.date_start, &params.date_end);

    let sql = format!(
        "SELECT date_str,
                COALESCE(SUM(ABS(money)), 0) as expense,
                0 as income
         FROM bill_merged{}
         WHERE status_str = '交易成功'
         GROUP BY date_str
         ORDER BY date_str",
        date_filter
    );

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(filter_params.iter()), |row| {
            Ok(DailyTrendItem {
                date: row.get(0)?,
                expense: row.get(1)?,
                income: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut trend = Vec::new();
    for row in rows {
        trend.push(row.map_err(|e| e.to_string())?);
    }

    Ok(trend)
}

#[tauri::command]
pub async fn get_category_distribution(
    state: State<'_, AppState>,
    params: StatisticsParams,
) -> Result<Vec<CategoryItem>, String> {
    let db = state.db_manager.read().await;
    let classifier = state.classifier.read().await;

    let conn = db
        .open_identity_db(params.identity_id)
        .map_err(|e| e.to_string())?;
    let (date_filter, filter_params) = build_date_filter(&params.date_start, &params.date_end);

    let sql = format!(
        "SELECT item_type, target_user, money FROM bill_merged{} AND status_str = '交易成功'",
        date_filter
    );

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(filter_params.iter()), |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, f64>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?;

    let mut category_map: std::collections::HashMap<String, (f64, u32)> =
        std::collections::HashMap::new();

    for row in rows {
        let (item_type, target_user, money) = row.map_err(|e| e.to_string())?;

        let category = if let Some(ref classifier) = *classifier {
            classifier
                .classify(
                    item_type.as_deref().unwrap_or(""),
                    target_user.as_deref().unwrap_or(""),
                    0,
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

    items.sort_by(|a, b| {
        b.value
            .partial_cmp(&a.value)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(items)
}

#[tauri::command]
pub async fn get_meal_distribution(
    state: State<'_, AppState>,
    params: StatisticsParams,
) -> Result<Vec<MealDistItem>, String> {
    let db = state.db_manager.read().await;
    let classifier = state.classifier.read().await;

    let conn = db
        .open_identity_db(params.identity_id)
        .map_err(|e| e.to_string())?;
    let (date_filter, filter_params) = build_date_filter(&params.date_start, &params.date_end);

    let sql = format!(
        "SELECT item_type, target_user, money, timestamp FROM bill_merged{} AND status_str = '交易成功'",
        date_filter
    );

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(filter_params.iter()), |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, f64>(2)?,
                row.get::<_, Option<i64>>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?;

    let mut meal_map: std::collections::HashMap<String, (u32, f64)> =
        std::collections::HashMap::new();

    for row in rows {
        let (item_type, target_user, money, timestamp) = row.map_err(|e| e.to_string())?;

        let meal = if let Some(ref classifier) = *classifier {
            classifier
                .classify(
                    item_type.as_deref().unwrap_or(""),
                    target_user.as_deref().unwrap_or(""),
                    timestamp.unwrap_or(0),
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

    items.sort_by(|a, b| {
        b.amount
            .partial_cmp(&a.amount)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(items)
}

/// 消费金额分布（直方图数据）
#[tauri::command]
pub async fn get_consumption_distribution(
    state: State<'_, AppState>,
    params: StatisticsParams,
) -> Result<Vec<ConsumptionBucketItem>, String> {
    let db = state.db_manager.read().await;

    let conn = db
        .open_identity_db(params.identity_id)
        .map_err(|e| e.to_string())?;
    let (date_filter, filter_params) = build_date_filter(&params.date_start, &params.date_end);

    let sql = format!(
        "SELECT ABS(money) as abs_money FROM bill_merged{} AND status_str = '交易成功'",
        date_filter
    );

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(filter_params.iter()), |row| {
            Ok(row.get::<_, f64>(0)?)
        })
        .map_err(|e| e.to_string())?;

    // 统计各金额区间
    let mut buckets = [
        ("<10元", 0u32, 0.0_f64),
        ("10-20元", 0u32, 0.0_f64),
        ("20-50元", 0u32, 0.0_f64),
        ("50-100元", 0u32, 0.0_f64),
        (">100元", 0u32, 0.0_f64),
    ];

    for row in rows {
        let money = row.map_err(|e| e.to_string())?;
        let idx = match money {
            m if m < 10.0 => 0,
            m if m < 20.0 => 1,
            m if m < 50.0 => 2,
            m if m < 100.0 => 3,
            _ => 4,
        };
        buckets[idx].1 += 1;
        buckets[idx].2 += money;
    }

    Ok(buckets
        .iter()
        .map(|(range, count, amount)| ConsumptionBucketItem {
            range: range.to_string(),
            count: *count,
            amount: *amount,
        })
        .collect())
}

/// 商户消费排行（按 target_user 分组，取 TOP10）
#[tauri::command]
pub async fn get_merchant_ranking(
    state: State<'_, AppState>,
    params: StatisticsParams,
) -> Result<Vec<MerchantRankingItem>, String> {
    let db = state.db_manager.read().await;

    let conn = db
        .open_identity_db(params.identity_id)
        .map_err(|e| e.to_string())?;
    let (date_filter, filter_params) = build_date_filter(&params.date_start, &params.date_end);

    let sql = format!(
        "SELECT target_user, SUM(ABS(money)) as total_amount, COUNT(*) as cnt
         FROM bill_merged{}
         WHERE status_str = '交易成功' AND target_user IS NOT NULL AND target_user != ''
         GROUP BY target_user
         ORDER BY total_amount DESC
         LIMIT 10",
        date_filter
    );

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(filter_params.iter()), |row| {
            Ok(MerchantRankingItem {
                merchant: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                amount: row.get::<_, f64>(1)?,
                count: row.get::<_, u32>(2)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut items = Vec::new();
    for row in rows {
        items.push(row.map_err(|e| e.to_string())?);
    }

    Ok(items)
}
