use serde::{Deserialize, Serialize};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
use tauri::State;

use crate::entity::bill_merged;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatisticsParams {
    pub identity_id: i64,
    pub date_start: Option<String>,
    pub date_end: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatisticsSummary {
    pub total_expense: f64,
    pub total_income: f64,
    pub net_expense: f64,
    pub daily_average: f64,
    pub expense_count: u32,
    pub income_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyTrendItem {
    pub date: String,
    pub expense: f64,
    pub income: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryItem {
    pub name: String,
    pub value: f64,
    pub count: u32,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MealDistItem {
    pub name: String,
    pub count: u32,
    pub amount: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsumptionBucketItem {
    pub range: String,
    pub count: u32,
    pub amount: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerchantRankingItem {
    pub merchant: String,
    pub count: u32,
    pub amount: f64,
}

const CATEGORY_COLORS: &[&str] = &[
    "#5B8FF9", "#5AD8A6", "#F6BD16", "#E86452", "#6DC8EC", "#945FB9", "#FF9845", "#1E9493",
    "#FF99C3", "#269A99",
];

fn apply_date_filter(
    mut query: sea_orm::Select<bill_merged::Entity>,
    date_start: &Option<String>,
    date_end: &Option<String>,
) -> sea_orm::Select<bill_merged::Entity> {
    if let Some(ref start) = date_start {
        tracing::debug!("[Statistics] date filter: start={}", start);
        query = query.filter(bill_merged::Column::DateStr.gte(start));
    }
    if let Some(ref end) = date_end {
        tracing::debug!("[Statistics] date filter: end={}", end);
        query = query.filter(bill_merged::Column::DateStr.lte(end));
    }
    query
}

fn success_query(identity_id: i64, date_start: &Option<String>, date_end: &Option<String>) -> sea_orm::Select<bill_merged::Entity> {
    tracing::debug!("[Statistics] success_query: identity_id={}, date_start={:?}, date_end={:?}",
        identity_id, date_start, date_end);
    let query = bill_merged::Entity::find()
        .filter(bill_merged::Column::IdentityId.eq(identity_id))
        .filter(bill_merged::Column::StatusStr.eq("交易成功"));
    apply_date_filter(query, date_start, date_end)
}

#[tauri::command]
pub async fn get_statistics_summary(
    state: State<'_, AppState>,
    params: StatisticsParams,
) -> Result<StatisticsSummary, String> {
    tracing::info!(
        "[Statistics] get_statistics_summary: identity_id={}, date_start={:?}, date_end={:?}",
        params.identity_id, params.date_start, params.date_end
    );

    let db = state.db_manager.read().await;
    let db_conn = db.db();

    let total_count = bill_merged::Entity::find()
        .filter(bill_merged::Column::IdentityId.eq(params.identity_id))
        .count(db_conn)
        .await
        .unwrap_or(0);
    tracing::info!("[Statistics] bill_merged total records for identity {}: {}", params.identity_id, total_count);

    let models = success_query(params.identity_id, &params.date_start, &params.date_end)
        .all(db_conn)
        .await
        .map_err(|e| {
            tracing::error!("[Statistics] summary fetch failed: {}", e);
            e.to_string()
        })?;

    let expense: f64 = models.iter().map(|m| m.money.unwrap_or(0.0).abs()).sum();
    let expense_count = models.len() as u32;
    let (income, income_count) = (0.0, 0);

    let days = {
        let unique_dates: std::collections::HashSet<&str> = models.iter()
            .map(|m| m.date_str.as_str())
            .collect();
        (unique_dates.len() as f64).max(1.0)
    };

    tracing::info!("[Statistics] expense={}, expense_count={}, days={}, daily_average={}",
        expense, expense_count, days, expense / days);

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
    tracing::info!(
        "[Statistics] get_daily_trend: identity_id={}, date_start={:?}, date_end={:?}",
        params.identity_id, params.date_start, params.date_end
    );

    let db = state.db_manager.read().await;
    let db_conn = db.db();

    let models = success_query(params.identity_id, &params.date_start, &params.date_end)
        .all(db_conn)
        .await
        .map_err(|e| {
            tracing::error!("[Statistics] daily_trend fetch failed: {}", e);
            e.to_string()
        })?;

    let mut daily_map: std::collections::BTreeMap<String, f64> = std::collections::BTreeMap::new();
    for m in &models {
        *daily_map.entry(m.date_str.clone()).or_default() += m.money.unwrap_or(0.0).abs();
    }

    tracing::info!("[Statistics] daily_trend: {} days computed", daily_map.len());
    for (date, expense) in &daily_map {
        tracing::debug!("[Statistics]   {} -> {}", date, expense);
    }

    Ok(daily_map
        .into_iter()
        .map(|(date, expense)| DailyTrendItem {
            date,
            expense,
            income: 0.0,
        })
        .collect())
}

#[tauri::command]
pub async fn get_category_distribution(
    state: State<'_, AppState>,
    params: StatisticsParams,
) -> Result<Vec<CategoryItem>, String> {
    tracing::info!(
        "[Statistics] get_category_distribution: identity_id={}, date_start={:?}, date_end={:?}",
        params.identity_id, params.date_start, params.date_end
    );

    let db = state.db_manager.read().await;
    let classifier = state.classifier.read().await;
    let db_conn = db.db();

    let has_classifier = classifier.is_some();
    tracing::info!("[Statistics] classifier loaded: {}", has_classifier);

    let models = success_query(params.identity_id, &params.date_start, &params.date_end)
        .all(db_conn)
        .await
        .map_err(|e| {
            tracing::error!("[Statistics] category fetch failed: {}", e);
            e.to_string()
        })?;

    tracing::info!("[Statistics] category: {} success records fetched", models.len());

    let mut category_map: std::collections::HashMap<String, (f64, u32)> =
        std::collections::HashMap::new();

    for m in &models {
        let category = if let Some(ref classifier) = *classifier {
            classifier
                .classify(
                    m.item_type.as_deref().unwrap_or(""),
                    m.target_user.as_deref().unwrap_or(""),
                    0,
                )
                .type_label
                .clone()
                .unwrap_or_else(|| "其他".to_string())
        } else {
            "其他".to_string()
        };

        let money = m.money.unwrap_or(0.0);
        let entry = category_map.entry(category).or_insert((0.0, 0));
        entry.0 += money.abs();
        entry.1 += 1;
    }

    for (i, m) in models.iter().take(5).enumerate() {
        tracing::debug!(
            "[Statistics] sample[{}]: item_type={:?}, target_user={:?}, money={:?}, status={:?}",
            i, m.item_type, m.target_user, m.money, m.status_str
        );
    }

    tracing::info!("[Statistics] category distribution: {} categories", category_map.len());
    for (name, (value, count)) in &category_map {
        tracing::debug!("[Statistics]   {} -> value={}, count={}", name, value, count);
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
    tracing::info!(
        "[Statistics] get_meal_distribution: identity_id={}, date_start={:?}, date_end={:?}",
        params.identity_id, params.date_start, params.date_end
    );

    let db = state.db_manager.read().await;
    let classifier = state.classifier.read().await;
    let db_conn = db.db();

    let models = success_query(params.identity_id, &params.date_start, &params.date_end)
        .all(db_conn)
        .await
        .map_err(|e| {
            tracing::error!("[Statistics] meal fetch failed: {}", e);
            e.to_string()
        })?;

    tracing::info!("[Statistics] meal: {} success records fetched", models.len());

    let mut meal_map: std::collections::HashMap<String, (u32, f64)> =
        std::collections::HashMap::new();

    for m in &models {
        let meal = if let Some(ref classifier) = *classifier {
            classifier
                .classify(
                    m.item_type.as_deref().unwrap_or(""),
                    m.target_user.as_deref().unwrap_or(""),
                    m.timestamp.unwrap_or(0),
                )
                .meal
                .clone()
                .unwrap_or_else(|| "非用餐时段".to_string())
        } else {
            "非用餐时段".to_string()
        };

        let money = m.money.unwrap_or(0.0);
        let entry = meal_map.entry(meal).or_insert((0, 0.0));
        entry.0 += 1;
        entry.1 += money.abs();
    }

    tracing::info!("[Statistics] meal distribution: {} periods", meal_map.len());
    for (name, (count, amount)) in &meal_map {
        tracing::debug!("[Statistics]   {} -> count={}, amount={}", name, count, amount);
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

#[tauri::command]
pub async fn get_consumption_distribution(
    state: State<'_, AppState>,
    params: StatisticsParams,
) -> Result<Vec<ConsumptionBucketItem>, String> {
    tracing::info!(
        "[Statistics] get_consumption_distribution: identity_id={}, date_start={:?}, date_end={:?}",
        params.identity_id, params.date_start, params.date_end
    );

    let db = state.db_manager.read().await;
    let db_conn = db.db();

    let models = success_query(params.identity_id, &params.date_start, &params.date_end)
        .all(db_conn)
        .await
        .map_err(|e| {
            tracing::error!("[Statistics] consumption fetch failed: {}", e);
            e.to_string()
        })?;

    tracing::info!("[Statistics] consumption: {} success records fetched", models.len());

    let mut buckets = [
        ("<10元", 0u32, 0.0_f64),
        ("10-20元", 0u32, 0.0_f64),
        ("20-50元", 0u32, 0.0_f64),
        ("50-100元", 0u32, 0.0_f64),
        (">100元", 0u32, 0.0_f64),
    ];

    for m in &models {
        let money = m.money.unwrap_or(0.0).abs();
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

    tracing::info!("[Statistics] consumption buckets:");
    for (range, count, amount) in &buckets {
        tracing::info!("[Statistics]   {} -> count={}, amount={}", range, count, amount);
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

#[tauri::command]
pub async fn get_merchant_ranking(
    state: State<'_, AppState>,
    params: StatisticsParams,
) -> Result<Vec<MerchantRankingItem>, String> {
    tracing::info!(
        "[Statistics] get_merchant_ranking: identity_id={}, date_start={:?}, date_end={:?}",
        params.identity_id, params.date_start, params.date_end
    );

    let db = state.db_manager.read().await;
    let db_conn = db.db();

    let models = success_query(params.identity_id, &params.date_start, &params.date_end)
        .all(db_conn)
        .await
        .map_err(|e| {
            tracing::error!("[Statistics] merchant_ranking fetch failed: {}", e);
            e.to_string()
        })?;

    let mut merchant_map: std::collections::HashMap<String, (f64, u32)> =
        std::collections::HashMap::new();

    for m in &models {
        let target = match m.target_user.as_ref() {
            Some(t) if !t.is_empty() => t,
            _ => continue,
        };
        let money = m.money.unwrap_or(0.0).abs();
        let entry = merchant_map.entry(target.clone()).or_insert((0.0, 0));
        entry.0 += money;
        entry.1 += 1;
    }

    let mut items: Vec<(String, f64, u32)> = merchant_map
        .into_iter()
        .map(|(name, (amount, count))| (name, amount, count))
        .collect();
    items.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    items.truncate(10);

    tracing::info!("[Statistics] merchant ranking: {} merchants returned", items.len());
    for (merchant, amount, count) in &items {
        tracing::info!("[Statistics]   {} -> amount={}, count={}", merchant, amount, count);
    }

    Ok(items
        .into_iter()
        .map(|(merchant, amount, count)| MerchantRankingItem {
            merchant,
            amount,
            count,
        })
        .collect())
}
