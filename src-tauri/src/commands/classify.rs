use serde::{Deserialize, Serialize};
use tauri::State;

use crate::classification::ClassificationRules;
use crate::state::AppState;

/// 翻译结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationResult {
    pub position: String,
    pub room: String,
}

/// 统计条目（按分类/位置分组）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifiedStatisticsItem {
    pub category: String,
    pub display_name: String,
    pub emoji: String,
    pub total_amount: f64,
    pub count: u32,
    pub position: Option<String>,
    pub room: Option<String>,
}

/// 翻译对方账户：target_user → (position, room)
/// 数据来源：本地 database/bill/position.json（不存在时从 GitHub 下载）
#[tauri::command]
pub async fn translate_target(
    state: State<'_, AppState>,
    target_user: String,
) -> Result<Option<TranslationResult>, String> {
    tracing::info!(
        "[Classify] translate_target called, target_user={}",
        target_user
    );

    let translator = state.db_file_manager.create_position_translator();
    let result = translator.translate(&target_user);
    let translated = result.map(|(position, room)| TranslationResult { position, room });

    tracing::info!("[Classify] translate_target result: {:?}", translated);
    Ok(translated)
}

/// 获取完整的分类规则（供前端动态加载，避免硬编码）
#[tauri::command]
pub async fn get_classification_rules(
    state: State<'_, AppState>,
) -> Result<ClassificationRules, String> {
    let rules_toml = state
        .db_file_manager
        .read_file("rules.toml")
        .map_err(|e| format!("加载 rules.toml 失败: {}", e))?;
    let rules: ClassificationRules =
        toml::from_str(&rules_toml).map_err(|e| format!("解析 rules.toml 失败: {}", e))?;
    Ok(rules)
}

/// 分类账单：根据 name（消费类型）和 target（对方账户）返回分类
/// 数据来源：本地 database/bill/type.json（不存在时从 GitHub 下载）
#[tauri::command]
pub async fn classify_bill(
    state: State<'_, AppState>,
    name: String,
    target: String,
) -> Result<String, String> {
    tracing::info!(
        "[Classify] classify_bill called, name={}, target={}",
        name,
        target
    );

    let type_toml = state
        .db_file_manager
        .load_type_toml()
        .map_err(|e| format!("加载 type.toml 失败: {}", e))?;
    let classifier = shmtu_cas::classifier::BillClassifier::from_toml(&type_toml)
        .map_err(|e| format!("解析 type.toml 失败: {}", e))?;

    let category = classifier.classify(&name, &target);
    let result = serde_json::to_string(&category).unwrap_or_else(|_| "other".to_string());

    tracing::info!("[Classify] classify_bill result: {:?}", category);
    Ok(result)
}

/// 获取按分类/位置分组的账单统计
#[tauri::command]
pub async fn get_bill_statistics(
    state: State<'_, AppState>,
    identity_id: i64,
) -> Result<Vec<ClassifiedStatisticsItem>, String> {
    tracing::info!(
        "[Classify] get_bill_statistics called, identity_id={}",
        identity_id
    );

    let db = state.db_manager.read().await;
    let db_conn = db.db();

    let type_toml = state
        .db_file_manager
        .load_type_toml()
        .map_err(|e| format!("加载 type.toml 失败: {}", e))?;
    let classifier = shmtu_cas::classifier::BillClassifier::from_toml(&type_toml)
        .map_err(|e| format!("解析 type.toml 失败: {}", e))?;

    use crate::entity::bill_merged;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    let models = bill_merged::Entity::find()
        .filter(bill_merged::Column::IdentityId.eq(identity_id))
        .filter(bill_merged::Column::StatusStr.eq("交易成功"))
        .all(db_conn)
        .await
        .map_err(|e| {
            tracing::error!("[Classify] get_bill_statistics query failed: {}", e);
            e.to_string()
        })?;

    tracing::info!(
        "[Classify] get_bill_statistics: {} records fetched",
        models.len()
    );

    let mut stats: std::collections::HashMap<String, ClassifiedStatisticsItem> =
        std::collections::HashMap::new();

    for m in &models {
        let item_type = m.item_type.as_deref().unwrap_or("");
        let target_user = m.target_user.as_deref().unwrap_or("");
        let money = m.money.unwrap_or(0.0).abs();
        let pos = m.position.as_deref();
        let rm = m.room.as_deref();

        let category = classifier.classify(item_type, target_user);
        let category_name = format!("{:?}", category);
        let display_name = category.display_name().to_string();
        let emoji = category.emoji().to_string();

        let entry = stats
            .entry(category_name.clone())
            .or_insert(ClassifiedStatisticsItem {
                category: category_name,
                display_name,
                emoji,
                total_amount: 0.0,
                count: 0,
                position: pos.map(|s| s.to_string()),
                room: rm.map(|s| s.to_string()),
            });
        entry.total_amount += money;
        entry.count += 1;
    }

    let mut items: Vec<ClassifiedStatisticsItem> = stats.into_values().collect();
    items.sort_by(|a, b| {
        b.total_amount
            .partial_cmp(&a.total_amount)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(items)
}
