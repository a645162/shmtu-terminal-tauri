use chrono::NaiveDate;
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::entity::bill_merged;
use crate::state::AppState;

/// 将浮点数四舍五入到指定小数位数。
fn round_to_n(value: f64, n: u32) -> f64 {
    let factor = 10f64.powi(n as i32);
    (value * factor).round() / factor
}

/// 统计查询通用参数
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatisticsParams {
    pub identity_id: i64,
    pub date_start: Option<String>,
    pub date_end: Option<String>,
}

/// 消费概览统计结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatisticsSummary {
    pub total_expense: f64,
    pub total_income: f64,
    pub net_expense: f64,
    pub daily_average: f64,
    pub expense_count: u32,
    pub income_count: u32,
}

/// 每日消费趋势条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyTrendItem {
    pub date: String,
    pub expense: f64,
    pub income: f64,
}

/// 消费分类分布条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryItem {
    pub name: String,
    pub value: f64,
    pub count: u32,
    pub color: String,
}

/// 用餐时段分布条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MealDistItem {
    pub name: String,
    pub count: u32,
    pub amount: f64,
}

/// 消费金额区间分布条目
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

/// 单个分类的详细统计参数
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategorySummaryParams {
    pub identity_id: i64,
    pub category: String,
    pub date_start: Option<String>,
    pub date_end: Option<String>,
}

/// 单个分类的详细统计结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategorySummary {
    pub category: String,
    pub total_amount: f64,
    pub count: u32,
    pub daily_average: f64,
    pub avg_per_transaction: f64,
}

const CATEGORY_COLORS: &[&str] = &[
    "#5B8FF9", "#5AD8A6", "#F6BD16", "#E86452", "#6DC8EC", "#945FB9", "#FF9845", "#1E9493",
    "#FF99C3", "#269A99",
];

const INCOME_KEYWORDS: &[&str] = &["充值", "冲正", "退款", "返还", "补偿"];

/// 解析日期字符串，支持 "YYYY.MM.DD"、"YYYY-MM-DD"、"YYYY/MM/DD" 三种格式。
fn parse_bill_date(date_str: &str) -> Option<NaiveDate> {
    ["%Y.%m.%d", "%Y-%m-%d", "%Y/%m/%d"]
        .iter()
        .find_map(|fmt| NaiveDate::parse_from_str(date_str, fmt).ok())
}

/// 按日期范围过滤账单模型列表。
fn filter_models_by_date(
    models: Vec<bill_merged::Model>,
    date_start: &Option<String>,
    date_end: &Option<String>,
) -> Vec<bill_merged::Model> {
    let start = date_start.as_deref().and_then(parse_bill_date);
    let end = date_end.as_deref().and_then(parse_bill_date);

    models
        .into_iter()
        .filter(|model| {
            let Some(date) = parse_bill_date(&model.date_str) else {
                return false;
            };
            if let Some(start_date) = start {
                if date < start_date {
                    return false;
                }
            }
            if let Some(end_date) = end {
                if date > end_date {
                    return false;
                }
            }
            true
        })
        .collect()
}

/// 构造"交易成功"状态的查询条件，仅统计成功的交易记录。
fn success_query(identity_id: i64) -> sea_orm::Select<bill_merged::Entity> {
    tracing::debug!("[Statistics] success_query: identity_id={}", identity_id);
    bill_merged::Entity::find()
        .filter(bill_merged::Column::IdentityId.eq(identity_id))
        .filter(bill_merged::Column::StatusStr.eq("交易成功"))
}

fn is_income(model: &bill_merged::Model) -> bool {
    let item_type = model.item_type.as_deref().unwrap_or("");
    let target_user = model.target_user.as_deref().unwrap_or("");
    INCOME_KEYWORDS
        .iter()
        .any(|keyword| item_type.contains(keyword) || target_user.contains(keyword))
}

/// 获取消费概览统计：总支出、日均、笔数等。
#[tauri::command]
pub async fn get_statistics_summary(
    state: State<'_, AppState>,
    params: StatisticsParams,
) -> Result<StatisticsSummary, String> {
    tracing::info!(
        "[Statistics] get_statistics_summary: identity_id={}, date_start={:?}, date_end={:?}",
        params.identity_id,
        params.date_start,
        params.date_end
    );

    let db = state.db_manager.read().await;
    let db_conn = db.db();

    let total_count = bill_merged::Entity::find()
        .filter(bill_merged::Column::IdentityId.eq(params.identity_id))
        .count(db_conn)
        .await
        .unwrap_or(0);
    tracing::info!(
        "[Statistics] bill_merged total records for identity {}: {}",
        params.identity_id,
        total_count
    );

    let models = success_query(params.identity_id)
        .all(db_conn)
        .await
        .map_err(|e| {
            tracing::error!("[Statistics] summary fetch failed: {}", e);
            e.to_string()
        })?;
    let models = filter_models_by_date(models, &params.date_start, &params.date_end);

    let mut expense = 0.0;
    let mut income = 0.0;
    let mut expense_count = 0u32;
    let mut income_count = 0u32;

    for model in &models {
        let amount = model.money.unwrap_or(0.0).abs();
        if is_income(model) {
            income += amount;
            income_count += 1;
        } else {
            expense += amount;
            expense_count += 1;
        }
    }

    let days = {
        let unique_dates: std::collections::HashSet<&str> =
            models.iter().map(|m| m.date_str.as_str()).collect();
        (unique_dates.len() as f64).max(1.0)
    };

    tracing::info!(
        "[Statistics] expense={}, expense_count={}, days={}, daily_average={}",
        expense,
        expense_count,
        days,
        expense / days
    );

    let config = state.config.read().await;
    let dp = config.decimal_places();

    Ok(StatisticsSummary {
        total_expense: round_to_n(expense, dp),
        total_income: round_to_n(income, dp),
        net_expense: round_to_n(expense - income, dp),
        daily_average: round_to_n(expense / days, dp),
        expense_count,
        income_count,
    })
}

/// 获取每日消费趋势，按日期聚合消费金额。
#[tauri::command]
pub async fn get_daily_trend(
    state: State<'_, AppState>,
    params: StatisticsParams,
) -> Result<Vec<DailyTrendItem>, String> {
    tracing::info!(
        "[Statistics] get_daily_trend: identity_id={}, date_start={:?}, date_end={:?}",
        params.identity_id,
        params.date_start,
        params.date_end
    );

    let db = state.db_manager.read().await;
    let db_conn = db.db();

    let models = success_query(params.identity_id)
        .all(db_conn)
        .await
        .map_err(|e| {
            tracing::error!("[Statistics] daily_trend fetch failed: {}", e);
            e.to_string()
        })?;
    let models = filter_models_by_date(models, &params.date_start, &params.date_end);

    let mut daily_map: std::collections::BTreeMap<String, f64> = std::collections::BTreeMap::new();
    for m in &models {
        *daily_map.entry(m.date_str.clone()).or_default() += m.money.unwrap_or(0.0).abs();
    }

    tracing::info!(
        "[Statistics] daily_trend: {} days computed",
        daily_map.len()
    );
    for (date, expense) in &daily_map {
        tracing::debug!("[Statistics]   {} -> {}", date, expense);
    }

    let config = state.config.read().await;
    let dp = config.decimal_places();

    Ok(daily_map
        .into_iter()
        .map(|(date, expense)| DailyTrendItem {
            date,
            expense: round_to_n(expense, dp),
            income: 0.0,
        })
        .collect())
}

/// 获取消费分类分布，基于分类器将交易归类后统计各类别金额和笔数。
#[tauri::command]
pub async fn get_category_distribution(
    state: State<'_, AppState>,
    params: StatisticsParams,
) -> Result<Vec<CategoryItem>, String> {
    tracing::info!(
        "[Statistics] get_category_distribution: identity_id={}, date_start={:?}, date_end={:?}",
        params.identity_id,
        params.date_start,
        params.date_end
    );

    let db = state.db_manager.read().await;
    let classifier = state.classifier.read().await;
    let db_conn = db.db();

    let has_classifier = classifier.is_some();
    tracing::info!("[Statistics] classifier loaded: {}", has_classifier);

    let models = success_query(params.identity_id)
        .all(db_conn)
        .await
        .map_err(|e| {
            tracing::error!("[Statistics] category fetch failed: {}", e);
            e.to_string()
        })?;
    let models = filter_models_by_date(models, &params.date_start, &params.date_end);

    tracing::info!(
        "[Statistics] category: {} success records fetched",
        models.len()
    );

    let mut category_map: std::collections::HashMap<String, (f64, u32)> =
        std::collections::HashMap::new();

    for m in &models {
        if is_income(m) {
            continue;
        }

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
            i,
            m.item_type,
            m.target_user,
            m.money,
            m.status_str
        );
    }

    tracing::info!(
        "[Statistics] category distribution: {} categories",
        category_map.len()
    );
    for (name, (value, count)) in &category_map {
        tracing::debug!(
            "[Statistics]   {} -> value={}, count={}",
            name,
            value,
            count
        );
    }

    let mut items: Vec<CategoryItem> = category_map
        .into_iter()
        .map(|(name, (value, count))| CategoryItem {
            name,
            value,
            count,
            color: String::new(),
        })
        .collect();

    items.sort_by(|a, b| {
        b.value
            .partial_cmp(&a.value)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for (i, item) in items.iter_mut().enumerate() {
        item.color = CATEGORY_COLORS[i % CATEGORY_COLORS.len()].to_string();
    }

    let config = state.config.read().await;
    let dp = config.decimal_places();
    for item in &mut items {
        item.value = round_to_n(item.value, dp);
    }

    Ok(items)
}

/// 获取用餐时段分布，基于分类器和时间戳判断早/中/晚餐时段。
#[tauri::command]
pub async fn get_meal_distribution(
    state: State<'_, AppState>,
    params: StatisticsParams,
) -> Result<Vec<MealDistItem>, String> {
    tracing::info!(
        "[Statistics] get_meal_distribution: identity_id={}, date_start={:?}, date_end={:?}",
        params.identity_id,
        params.date_start,
        params.date_end
    );

    let db = state.db_manager.read().await;
    let classifier = state.classifier.read().await;
    let db_conn = db.db();

    let models = success_query(params.identity_id)
        .all(db_conn)
        .await
        .map_err(|e| {
            tracing::error!("[Statistics] meal fetch failed: {}", e);
            e.to_string()
        })?;
    let models = filter_models_by_date(models, &params.date_start, &params.date_end);

    tracing::info!(
        "[Statistics] meal: {} success records fetched",
        models.len()
    );

    let mut meal_map: std::collections::HashMap<String, (u32, f64)> =
        std::collections::HashMap::new();

    for m in &models {
        if is_income(m) {
            continue;
        }

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
        tracing::debug!(
            "[Statistics]   {} -> count={}, amount={}",
            name,
            count,
            amount
        );
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

    let config = state.config.read().await;
    let dp = config.decimal_places();
    for item in &mut items {
        item.amount = round_to_n(item.amount, dp);
    }

    Ok(items)
}

/// 获取消费金额区间分布，按 5 个金额档位统计笔数和总额。
#[tauri::command]
pub async fn get_consumption_distribution(
    state: State<'_, AppState>,
    params: StatisticsParams,
) -> Result<Vec<ConsumptionBucketItem>, String> {
    tracing::info!(
        "[Statistics] get_consumption_distribution: identity_id={}, date_start={:?}, date_end={:?}",
        params.identity_id,
        params.date_start,
        params.date_end
    );

    let db = state.db_manager.read().await;
    let db_conn = db.db();

    let models = success_query(params.identity_id)
        .all(db_conn)
        .await
        .map_err(|e| {
            tracing::error!("[Statistics] consumption fetch failed: {}", e);
            e.to_string()
        })?;
    let models = filter_models_by_date(models, &params.date_start, &params.date_end);

    tracing::info!(
        "[Statistics] consumption: {} success records fetched",
        models.len()
    );

    let mut buckets = [
        ("<10元", 0u32, 0.0_f64),
        ("10-20元", 0u32, 0.0_f64),
        ("20-50元", 0u32, 0.0_f64),
        ("50-100元", 0u32, 0.0_f64),
        (">100元", 0u32, 0.0_f64),
    ];

    for m in &models {
        if is_income(m) {
            continue;
        }

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
        tracing::info!(
            "[Statistics]   {} -> count={}, amount={}",
            range,
            count,
            amount
        );
    }

    let config = state.config.read().await;
    let dp = config.decimal_places();

    Ok(buckets
        .iter()
        .map(|(range, count, amount)| ConsumptionBucketItem {
            range: range.to_string(),
            count: *count,
            amount: round_to_n(*amount, dp),
        })
        .collect())
}

/// 获取商户消费排行，按对方账户聚合后取消费金额前 10 名。
#[tauri::command]
pub async fn get_merchant_ranking(
    state: State<'_, AppState>,
    params: StatisticsParams,
) -> Result<Vec<MerchantRankingItem>, String> {
    tracing::info!(
        "[Statistics] get_merchant_ranking: identity_id={}, date_start={:?}, date_end={:?}",
        params.identity_id,
        params.date_start,
        params.date_end
    );

    let db = state.db_manager.read().await;
    let db_conn = db.db();

    let models = success_query(params.identity_id)
        .all(db_conn)
        .await
        .map_err(|e| {
            tracing::error!("[Statistics] merchant_ranking fetch failed: {}", e);
            e.to_string()
        })?;
    let models = filter_models_by_date(models, &params.date_start, &params.date_end);

    let mut merchant_map: std::collections::HashMap<String, (f64, u32)> =
        std::collections::HashMap::new();

    for m in &models {
        if is_income(m) {
            continue;
        }

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

    tracing::info!(
        "[Statistics] merchant ranking: {} merchants returned",
        items.len()
    );
    for (merchant, amount, count) in &items {
        tracing::info!(
            "[Statistics]   {} -> amount={}, count={}",
            merchant,
            amount,
            count
        );
    }

    let config = state.config.read().await;
    let dp = config.decimal_places();

    Ok(items
        .into_iter()
        .map(|(merchant, amount, count)| MerchantRankingItem {
            merchant,
            amount: round_to_n(amount, dp),
            count,
        })
        .collect())
}

/// 忘记拔卡统计条目
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgotCardItem {
    pub id: i64,
    pub date: String,
    pub time: String,
    pub amount: f64,
    pub target_user: String,
}

/// 忘记拔卡统计结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgotCardStats {
    pub count: u32,
    pub total_amount: f64,
    pub items: Vec<ForgotCardItem>,
}

/// 获取"忘记拔卡"统计：洗澡消费恰好为5元的记录。
///
/// 洗澡上限为5元，消费5元意味着水龙头一直开着（忘记拔卡）。
#[tauri::command]
pub async fn get_forgot_card_stats(
    state: State<'_, AppState>,
    params: StatisticsParams,
) -> Result<ForgotCardStats, String> {
    tracing::info!(
        "[Statistics] get_forgot_card_stats: identity_id={}, date_start={:?}, date_end={:?}",
        params.identity_id,
        params.date_start,
        params.date_end
    );

    let db = state.db_manager.read().await;
    let classifier = state.classifier.read().await;
    let db_conn = db.db();

    let models = success_query(params.identity_id)
        .all(db_conn)
        .await
        .map_err(|e| e.to_string())?;
    let models = filter_models_by_date(models, &params.date_start, &params.date_end);

    let mut items: Vec<ForgotCardItem> = Vec::new();

    for m in &models {
        if is_income(m) {
            continue;
        }

        let money = m.money.unwrap_or(0.0).abs();

        if (money - 5.0).abs() > 0.01 {
            continue;
        }

        let category = if let Some(ref classifier) = *classifier {
            classifier
                .classify(
                    m.item_type.as_deref().unwrap_or(""),
                    m.target_user.as_deref().unwrap_or(""),
                    0,
                )
                .type_label
                .clone()
                .unwrap_or_default()
        } else {
            let target = m.target_user.as_deref().unwrap_or("");
            if target.contains("淋浴") || target.contains("热水") {
                "洗澡".to_string()
            } else {
                String::new()
            }
        };

        if category != "洗澡" {
            continue;
        }

        items.push(ForgotCardItem {
            id: m.id,
            date: m.date_str.clone(),
            time: m.time_str_formatted.clone().unwrap_or_default(),
            amount: money,
            target_user: m.target_user.clone().unwrap_or_default(),
        });
    }

    let count = items.len() as u32;
    let total_amount: f64 = items.iter().map(|i| i.amount).sum();

    let config = state.config.read().await;
    let dp = config.decimal_places();

    tracing::info!(
        "[Statistics] forgot_card_stats: count={}, total_amount={}",
        count,
        total_amount
    );

    Ok(ForgotCardStats {
        count,
        total_amount: round_to_n(total_amount, dp),
        items,
    })
}

/// 获取指定分类的消费明细统计：总额、笔数、日均、笔均。
#[tauri::command]
pub async fn get_category_summary(
    state: State<'_, AppState>,
    params: CategorySummaryParams,
) -> Result<CategorySummary, String> {
    tracing::info!(
        "[Statistics] get_category_summary: identity_id={}, category={}, date_start={:?}, date_end={:?}",
        params.identity_id, params.category, params.date_start, params.date_end
    );

    let db = state.db_manager.read().await;
    let classifier = state.classifier.read().await;
    let db_conn = db.db();

    let models = success_query(params.identity_id)
        .all(db_conn)
        .await
        .map_err(|e| {
            tracing::error!("[Statistics] category_summary fetch failed: {}", e);
            e.to_string()
        })?;
    let models = filter_models_by_date(models, &params.date_start, &params.date_end);

    let category_name = params.category.clone();
    let mut total_amount = 0.0_f64;
    let mut count = 0u32;

    for m in &models {
        if is_income(m) {
            continue;
        }

        let cat = if let Some(ref classifier) = *classifier {
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

        if cat == category_name {
            total_amount += m.money.unwrap_or(0.0).abs();
            count += 1;
        }
    }

    let days = {
        let unique_dates: std::collections::HashSet<&str> =
            models.iter().map(|m| m.date_str.as_str()).collect();
        (unique_dates.len() as f64).max(1.0)
    };

    let avg_per_transaction = if count > 0 {
        total_amount / count as f64
    } else {
        0.0
    };

    tracing::info!(
        "[Statistics] category_summary: category={}, total={}, count={}, daily_avg={}, per_txn={}",
        category_name,
        total_amount,
        count,
        total_amount / days,
        avg_per_transaction
    );

    let config = state.config.read().await;
    let dp = config.decimal_places();

    Ok(CategorySummary {
        category: category_name,
        total_amount: round_to_n(total_amount, dp),
        count,
        daily_average: round_to_n(total_amount / days, dp),
        avg_per_transaction: round_to_n(avg_per_transaction, dp),
    })
}

/// 分类账单条目
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryBillItem {
    pub id: i64,
    pub date: String,
    pub time: String,
    pub item_type: String,
    pub target_user: String,
    pub amount: f64,
    pub method: String,
}

/// 获取指定分类下的账单明细列表。
#[tauri::command]
pub async fn get_category_bills(
    state: State<'_, AppState>,
    params: CategorySummaryParams,
) -> Result<Vec<CategoryBillItem>, String> {
    tracing::info!(
        "[Statistics] get_category_bills: identity_id={}, category={}, date_start={:?}, date_end={:?}",
        params.identity_id, params.category, params.date_start, params.date_end
    );

    let db = state.db_manager.read().await;
    let classifier = state.classifier.read().await;
    let db_conn = db.db();

    let models = success_query(params.identity_id)
        .all(db_conn)
        .await
        .map_err(|e| e.to_string())?;
    let models = filter_models_by_date(models, &params.date_start, &params.date_end);

    let category_name = params.category.clone();
    let mut items: Vec<CategoryBillItem> = Vec::new();

    for m in &models {
        if is_income(m) {
            continue;
        }

        let cat = if let Some(ref classifier) = *classifier {
            classifier
                .classify(
                    m.item_type.as_deref().unwrap_or(""),
                    m.target_user.as_deref().unwrap_or(""),
                    0,
                )
                .type_label
                .clone()
                .unwrap_or_default()
        } else {
            String::new()
        };

        if cat != category_name {
            continue;
        }

        items.push(CategoryBillItem {
            id: m.id,
            date: m.date_str.clone(),
            time: m.time_str_formatted.clone().unwrap_or_default(),
            item_type: m.item_type.clone().unwrap_or_default(),
            target_user: m.target_user.clone().unwrap_or_default(),
            amount: m.money.unwrap_or(0.0).abs(),
            method: m.method.clone().unwrap_or_default(),
        });
    }

    tracing::info!(
        "[Statistics] get_category_bills: category={}, returned {} items",
        category_name,
        items.len()
    );

    Ok(items)
}
