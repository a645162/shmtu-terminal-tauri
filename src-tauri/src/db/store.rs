use std::collections::{HashMap, HashSet};

use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, Set,
};
use shmtu_cas::classifier::PositionTranslator;
use shmtu_cas::datatype::bill::BillItem;
use crate::db::init::{
    bill_merged_model_to_app, bill_original_model_to_app, operation_log_model_to_app,
};
use crate::entity::*;
use crate::error::AppResult;
use crate::models::{BillMerged, BillOriginal, OperationLog};

pub struct BillStoreImpl {
    db: DatabaseConnection,
    account_id: String,
    identity_id: i64,
    known_numbers: HashSet<String>,
    pending_bills: Vec<BillItem>,
    translator: PositionTranslator,
}

impl BillStoreImpl {
    pub async fn new(
        db: DatabaseConnection,
        account_id: &str,
        identity_id: i64,
        translator: PositionTranslator,
    ) -> AppResult<Self> {
        let known_numbers = Self::load_known_numbers(&db, account_id, identity_id).await?;
        Ok(Self {
            db,
            account_id: account_id.to_string(),
            identity_id,
            known_numbers,
            pending_bills: Vec::new(),
            translator,
        })
    }

    async fn load_known_numbers(
        db: &DatabaseConnection,
        account_id: &str,
        identity_id: i64,
    ) -> AppResult<HashSet<String>> {
        let mut numbers = HashSet::new();

        let rows = bill_original::Entity::find()
            .filter(bill_original::Column::AccountId.eq(account_id))
            .all(db)
            .await?;
        for row in rows {
            for number in normalized_numbers_from_fields(
                row.number.as_deref(),
                row.item_type.as_deref(),
                row.number_list.as_deref(),
            ) {
                numbers.insert(number);
            }
        }

        let rows = bill_merged::Entity::find()
            .filter(bill_merged::Column::IdentityId.eq(identity_id))
            .all(db)
            .await?;
        for row in rows {
            for number in normalized_numbers_from_fields(
                row.number.as_deref(),
                row.item_type.as_deref(),
                row.number_list.as_deref(),
            ) {
                numbers.insert(number);
            }
        }

        Ok(numbers)
    }

    pub async fn save_bill_original(&self, bill: &BillItem, now: &str) -> AppResult<()> {
        let number_list_json = serde_json::to_string(&bill.number_list)?;
        let model = bill_original::ActiveModel {
            date_str: Set(bill.date_str.clone()),
            time_str: Set(bill.time_str.clone()),
            time_str_formatted: Set(Some(bill.time_str_formatted.clone())),
            date_time_formatted: Set(Some(bill.date_time_formatted.clone())),
            end_date_time_formatted: Set(Some(bill.end_date_time_formatted.clone())),
            timestamp: Set(Some(bill.timestamp)),
            end_timestamp: Set(Some(bill.end_timestamp)),
            item_type: Set(Some(bill.item_type.clone())),
            number: Set(Some(bill.number.clone())),
            number_list: Set(Some(number_list_json)),
            target_user: Set(Some(bill.target_user.clone())),
            money_str: Set(Some(bill.money_str.clone())),
            money: Set(Some(bill.money as f64)),
            method: Set(Some(bill.method.clone())),
            status_str: Set(Some(bill.status_str.clone())),
            is_combined: Set(bill.is_combined),
            account_id: Set(self.account_id.clone()),
            synced_at: Set(Some(now.to_string())),
            ..Default::default()
        };
        bill_original::Entity::insert(model).exec(&self.db).await?;
        Ok(())
    }

    pub async fn append_to_merged(&self, bill: &BillItem, now: &str) -> AppResult<()> {
        let number_list_json = serde_json::to_string(&bill.number_list)?;

        let (position, room) = self.resolve_position_and_room(&bill.target_user);

        let model = bill_merged::ActiveModel {
            identity_id: Set(self.identity_id),
            date_str: Set(bill.date_str.clone()),
            time_str: Set(bill.time_str.clone()),
            time_str_formatted: Set(Some(bill.time_str_formatted.clone())),
            date_time_formatted: Set(Some(bill.date_time_formatted.clone())),
            end_date_time_formatted: Set(Some(bill.end_date_time_formatted.clone())),
            timestamp: Set(Some(bill.timestamp)),
            end_timestamp: Set(Some(bill.end_timestamp)),
            item_type: Set(Some(bill.item_type.clone())),
            number: Set(Some(bill.number.clone())),
            number_list: Set(Some(number_list_json)),
            target_user: Set(Some(bill.target_user.clone())),
            money_str: Set(Some(bill.money_str.clone())),
            money: Set(Some(bill.money as f64)),
            method: Set(Some(bill.method.clone())),
            status_str: Set(Some(bill.status_str.clone())),
            is_combined: Set(bill.is_combined),
            source_account_id: Set(Some(self.account_id.clone())),
            is_manual: Set(false),
            position: Set(Some(position)),
            room: Set(Some(room)),
            notes: Set(None),
            synced_at: Set(Some(now.to_string())),
            ..Default::default()
        };
        bill_merged::Entity::insert(model).exec(&self.db).await?;
        Ok(())
    }

    fn resolve_position_and_room(&self, target_user: &str) -> (String, String) {
        self.translator
            .translate(target_user)
            .unwrap_or_else(|| self.translator.translate_or_raw(target_user))
    }

    pub async fn backfill_merged_metadata(&self, identity_id: i64) -> AppResult<usize> {
        use sea_orm::{ActiveModelTrait, IntoActiveModel};

        let candidates = bill_merged::Entity::find()
            .filter(bill_merged::Column::IdentityId.eq(identity_id))
            .filter(
                Condition::any()
                    .add(bill_merged::Column::Number.is_null())
                    .add(bill_merged::Column::Number.eq(""))
                    .add(bill_merged::Column::Position.is_null())
                    .add(bill_merged::Column::Position.eq(""))
                    .add(bill_merged::Column::Room.is_null())
                    .add(bill_merged::Column::Room.eq(""))
                    .add(bill_merged::Column::ItemType.contains("交易号")),
            )
            .all(&self.db)
            .await?;

        let mut updated = 0usize;

        for model in candidates {
            let normalized_item_type =
                model.item_type.as_deref().map(normalize_item_type).filter(|s| !s.is_empty());
            let normalized_number = normalize_number(
                model.number.as_deref(),
                model.item_type.as_deref(),
                model.number_list.as_deref(),
            );
            let normalized_number_list =
                normalize_number_list_json(model.number_list.as_deref(), normalized_number.as_deref());
            let normalized_target_user = model
                .target_user
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            let normalized_position_room = normalized_target_user
                .as_deref()
                .map(|target_user| self.resolve_position_and_room(target_user));

            let needs_update =
                normalized_item_type != model.item_type
                    || normalized_number != model.number
                    || normalized_number_list != model.number_list
                    || normalized_target_user != model.target_user
                    || normalized_position_room.as_ref().map(|(p, _)| p) != model.position.as_ref()
                    || normalized_position_room.as_ref().map(|(_, r)| r) != model.room.as_ref();

            if !needs_update {
                continue;
            }

            let mut active = model.into_active_model();
            active.item_type = Set(normalized_item_type);
            active.number = Set(normalized_number);
            active.number_list = Set(normalized_number_list);
            active.target_user = Set(normalized_target_user);
            active.position = Set(normalized_position_room.as_ref().map(|(p, _)| p.clone()));
            active.room = Set(normalized_position_room.as_ref().map(|(_, r)| r.clone()));
            active.update(&self.db).await?;
            updated += 1;
        }

        Ok(updated)
    }

    pub async fn backfill_original_metadata(&self, account_id: &str) -> AppResult<usize> {
        use sea_orm::{ActiveModelTrait, IntoActiveModel};

        let candidates = bill_original::Entity::find()
            .filter(bill_original::Column::AccountId.eq(account_id))
            .filter(
                Condition::any()
                    .add(bill_original::Column::Number.is_null())
                    .add(bill_original::Column::Number.eq(""))
                    .add(bill_original::Column::ItemType.contains("交易号")),
            )
            .all(&self.db)
            .await?;

        let mut updated = 0usize;

        for model in candidates {
            let normalized_item_type =
                model.item_type.as_deref().map(normalize_item_type).filter(|s| !s.is_empty());
            let normalized_number = normalize_number(
                model.number.as_deref(),
                model.item_type.as_deref(),
                model.number_list.as_deref(),
            );
            let normalized_number_list =
                normalize_number_list_json(model.number_list.as_deref(), normalized_number.as_deref());

            let needs_update = normalized_item_type != model.item_type
                || normalized_number != model.number
                || normalized_number_list != model.number_list;

            if !needs_update {
                continue;
            }

            let mut active = model.into_active_model();
            active.item_type = Set(normalized_item_type);
            active.number = Set(normalized_number);
            active.number_list = Set(normalized_number_list);
            active.update(&self.db).await?;
            updated += 1;
        }

        Ok(updated)
    }

    pub async fn dedupe_merged_by_identity(&self, identity_id: i64) -> AppResult<usize> {
        let models = bill_merged::Entity::find()
            .filter(bill_merged::Column::IdentityId.eq(identity_id))
            .order_by_asc(bill_merged::Column::Id)
            .all(&self.db)
            .await?;

        let mut grouped: HashMap<String, Vec<bill_merged::Model>> = HashMap::new();
        for model in models {
            let Some(number) = model.number.as_deref().map(digits_only).filter(|n| !n.is_empty()) else {
                continue;
            };
            grouped.entry(number).or_default().push(model);
        }

        let duplicate_ids = grouped
            .into_values()
            .flat_map(|models| duplicate_merged_ids(models))
            .collect::<Vec<_>>();

        if duplicate_ids.is_empty() {
            return Ok(0);
        }

        bill_merged::Entity::delete_many()
            .filter(bill_merged::Column::Id.is_in(duplicate_ids.clone()))
            .exec(&self.db)
            .await?;

        self.log_operation(
            "merge",
            &serde_json::to_string(&duplicate_ids).unwrap_or_default(),
            &format!("身份账单去重，删除 {} 条重复记录", duplicate_ids.len()),
            None,
        )
        .await?;

        Ok(duplicate_ids.len())
    }

    pub async fn dedupe_original_by_account(&self, account_id: &str) -> AppResult<usize> {
        let models = bill_original::Entity::find()
            .filter(bill_original::Column::AccountId.eq(account_id))
            .order_by_asc(bill_original::Column::Id)
            .all(&self.db)
            .await?;

        let mut grouped: HashMap<String, Vec<bill_original::Model>> = HashMap::new();
        for model in models {
            let Some(number) = model.number.as_deref().map(digits_only).filter(|n| !n.is_empty()) else {
                continue;
            };
            grouped.entry(number).or_default().push(model);
        }

        let duplicate_ids = grouped
            .into_values()
            .flat_map(|models| duplicate_original_ids(models))
            .collect::<Vec<_>>();

        if duplicate_ids.is_empty() {
            return Ok(0);
        }

        bill_original::Entity::delete_many()
            .filter(bill_original::Column::Id.is_in(duplicate_ids.clone()))
            .exec(&self.db)
            .await?;

        self.log_operation(
            "merge",
            &serde_json::to_string(&duplicate_ids).unwrap_or_default(),
            &format!(
                "账号原始账单去重（{}），删除 {} 条重复记录",
                account_id,
                duplicate_ids.len()
            ),
            Some(account_id),
        )
        .await?;

        Ok(duplicate_ids.len())
    }

    pub async fn flush_pending_bills(&mut self) -> AppResult<()> {
        if self.pending_bills.is_empty() {
            return Ok(());
        }

        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let pending_bills = std::mem::take(&mut self.pending_bills);

        for bill in &pending_bills {
            self.save_bill_original(bill, &now).await?;
            self.append_to_merged(bill, &now).await?;
        }

        Ok(())
    }

    pub async fn list_original_bills(
        &self,
        account_id: &str,
        page: u32,
        page_size: u32,
    ) -> AppResult<(Vec<BillOriginal>, u32)> {
        let paginator = bill_original::Entity::find()
            .filter(bill_original::Column::AccountId.eq(account_id))
            .order_by_desc(bill_original::Column::Timestamp)
            .paginate(&self.db, page_size as u64);

        let total = paginator.num_items().await? as u32;
        let total_pages = total.div_ceil(page_size);

        let models = paginator.fetch_page(page as u64).await?;
        let bills: Vec<BillOriginal> = models
            .into_iter()
            .map(bill_original_model_to_app)
            .collect();

        Ok((bills, total_pages))
    }

    pub async fn list_merged_bills(
        &self,
        identity_id: i64,
        page: u32,
        page_size: u32,
    ) -> AppResult<(Vec<BillMerged>, u32)> {
        let paginator = bill_merged::Entity::find()
            .filter(bill_merged::Column::IdentityId.eq(identity_id))
            .order_by_desc(bill_merged::Column::Timestamp)
            .paginate(&self.db, page_size as u64);

        let total = paginator.num_items().await? as u32;
        let total_pages = if page_size > 0 {
            total.div_ceil(page_size)
        } else {
            1
        };

        let models = paginator.fetch_page(page as u64).await?;
        let bills: Vec<BillMerged> = models
            .into_iter()
            .map(bill_merged_model_to_app)
            .collect();

        Ok((bills, total_pages))
    }

    pub async fn add_manual_bill(&self, identity_id: i64, bill: &BillItem) -> AppResult<i64> {
        let number_list_json = serde_json::to_string(&bill.number_list)?;
        let now = chrono::Local::now().to_rfc3339();
        let (position, room) = self.resolve_position_and_room(&bill.target_user);

        let model = bill_merged::ActiveModel {
            identity_id: Set(identity_id),
            date_str: Set(bill.date_str.clone()),
            time_str: Set(bill.time_str.clone()),
            time_str_formatted: Set(Some(bill.time_str_formatted.clone())),
            date_time_formatted: Set(Some(bill.date_time_formatted.clone())),
            end_date_time_formatted: Set(Some(bill.end_date_time_formatted.clone())),
            timestamp: Set(Some(bill.timestamp)),
            end_timestamp: Set(Some(bill.end_timestamp)),
            item_type: Set(Some(bill.item_type.clone())),
            number: Set(Some(bill.number.clone())),
            number_list: Set(Some(number_list_json.clone())),
            target_user: Set(Some(bill.target_user.clone())),
            money_str: Set(Some(bill.money_str.clone())),
            money: Set(Some(bill.money as f64)),
            method: Set(Some(bill.method.clone())),
            status_str: Set(Some(bill.status_str.clone())),
            is_combined: Set(bill.is_combined),
            source_account_id: Set(None),
            is_manual: Set(true),
            position: Set(Some(position)),
            room: Set(Some(room)),
            notes: Set(None),
            synced_at: Set(Some(now.clone())),
            ..Default::default()
        };

        let result = bill_merged::Entity::insert(model).exec(&self.db).await?;
        let id = result.last_insert_id;

        self.log_operation(
            "add",
            &number_list_json,
            &format!(
                "手动添加账单: {} {}",
                bill.date_time_formatted, bill.item_type
            ),
            None,
        )
        .await?;

        Ok(id)
    }

    pub async fn update_bill_notes(&self, bill_id: i64, notes: Option<String>) -> AppResult<()> {
        use sea_orm::{ActiveModelTrait, IntoActiveModel};
        let model = bill_merged::Entity::find_by_id(bill_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| crate::error::AppError::Database("账单不存在".to_string()))?;
        let mut active = model.into_active_model();
        active.notes = Set(notes);
        active.update(&self.db).await?;
        Ok(())
    }

    pub async fn delete_merged_bill(&self, _identity_id: i64, bill_id: i64) -> AppResult<()> {
        let number_list = bill_merged::Entity::find_by_id(bill_id)
            .one(&self.db)
            .await?
            .and_then(|m| m.number_list);

        bill_merged::Entity::delete_by_id(bill_id)
            .exec(&self.db)
            .await?;

        if let Some(nl) = number_list {
            self.log_operation(
                "delete",
                &nl,
                &format!("手动删除账单 ID={}", bill_id),
                None,
            )
            .await?;
        }

        Ok(())
    }

    async fn log_operation(
        &self,
        operation_type: &str,
        record_numbers: &str,
        description: &str,
        account_id: Option<&str>,
    ) -> AppResult<()> {
        let now = chrono::Local::now().to_rfc3339();
        let model = operation_log::ActiveModel {
            identity_id: Set(self.identity_id),
            operation_type: Set(operation_type.to_string()),
            record_numbers: Set(Some(record_numbers.to_string())),
            operation_time: Set(now),
            description: Set(Some(description.to_string())),
            account_id: Set(account_id.map(|s| s.to_string())),
            ..Default::default()
        };
        operation_log::Entity::insert(model).exec(&self.db).await?;
        Ok(())
    }

    pub async fn list_operation_logs(&self, identity_id: i64) -> AppResult<Vec<OperationLog>> {
        let models = operation_log::Entity::find()
            .filter(operation_log::Column::IdentityId.eq(identity_id))
            .order_by_desc(operation_log::Column::Id)
            .all(&self.db)
            .await?;

        Ok(models.into_iter().map(operation_log_model_to_app).collect())
    }

    pub async fn get_all_merged_bills(&self, identity_id: i64) -> AppResult<Vec<BillMerged>> {
        let models = bill_merged::Entity::find()
            .filter(bill_merged::Column::IdentityId.eq(identity_id))
            .order_by_asc(bill_merged::Column::Timestamp)
            .all(&self.db)
            .await?;

        Ok(models.into_iter().map(bill_merged_model_to_app).collect())
    }

    pub async fn get_all_original_bills(&self, account_id: &str) -> AppResult<Vec<BillOriginal>> {
        let models = bill_original::Entity::find()
            .filter(bill_original::Column::AccountId.eq(account_id))
            .order_by_asc(bill_original::Column::Timestamp)
            .all(&self.db)
            .await?;

        Ok(models.into_iter().map(bill_original_model_to_app).collect())
    }
}

impl shmtu_cas::sync::BillStore for BillStoreImpl {
    fn contains(&self, number: &str) -> bool {
        self.known_numbers.contains(number)
    }

    fn merge(&mut self, new_bills: Vec<BillItem>) {
        if new_bills.is_empty() {
            return;
        }

        let mut deduped_bills = Vec::with_capacity(new_bills.len());

        for bill in new_bills {
            let numbers = candidate_bill_numbers(&bill);
            if !numbers.is_empty() && numbers.iter().any(|number| self.known_numbers.contains(number)) {
                continue;
            }

            for number in &numbers {
                self.known_numbers.insert(number.clone());
            }
            deduped_bills.push(bill);
        }

        self.pending_bills.extend(deduped_bills);
    }
}

fn normalize_item_type(raw: &str) -> String {
    let compact = compact_whitespace(raw);
    let before_marker = compact.split("交易号").next().unwrap_or(&compact).trim();
    before_marker
        .trim_end_matches([':', '：'])
        .trim()
        .to_string()
}

fn normalize_number(
    raw_number: Option<&str>,
    raw_item_type: Option<&str>,
    raw_number_list: Option<&str>,
) -> Option<String> {
    let number = digits_only(raw_number.unwrap_or_default());
    if !number.is_empty() {
        return Some(number);
    }

    if let Some(number) = raw_item_type.and_then(extract_transaction_number) {
        return Some(number);
    }

    raw_number_list
        .and_then(first_number_from_number_list)
        .filter(|number| !number.is_empty())
}

fn normalize_number_list_json(raw_number_list: Option<&str>, number: Option<&str>) -> Option<String> {
    let mut numbers = Vec::new();

    if let Some(raw_number_list) = raw_number_list {
        if let Ok(list) = serde_json::from_str::<Vec<String>>(raw_number_list) {
            for item in list {
                let digits = digits_only(&item);
                if !digits.is_empty() && !numbers.contains(&digits) {
                    numbers.push(digits);
                }
            }
        }
    }

    if let Some(number) = number {
        if !number.is_empty() && !numbers.iter().any(|existing| existing == number) {
            numbers.push(number.to_string());
        }
    }

    if numbers.is_empty() {
        None
    } else {
        serde_json::to_string(&numbers).ok()
    }
}

fn first_number_from_number_list(raw_number_list: &str) -> Option<String> {
    serde_json::from_str::<Vec<String>>(raw_number_list)
        .ok()?
        .into_iter()
        .map(|item| digits_only(&item))
        .find(|item| !item.is_empty())
}

fn extract_transaction_number(raw: &str) -> Option<String> {
    let marker_index = raw.find("交易号")?;
    let number = digits_only(&raw[marker_index..]);
    if number.is_empty() {
        None
    } else {
        Some(number)
    }
}

fn digits_only(raw: &str) -> String {
    raw.chars().filter(|ch| ch.is_ascii_digit()).collect()
}

fn compact_whitespace(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn candidate_bill_numbers(bill: &BillItem) -> Vec<String> {
    let mut numbers = bill
        .number_list
        .iter()
        .map(|number| digits_only(number))
        .filter(|number| !number.is_empty())
        .collect::<Vec<_>>();

    let direct_number = digits_only(&bill.number);
    if !direct_number.is_empty() && !numbers.iter().any(|number| number == &direct_number) {
        numbers.push(direct_number);
    }

    numbers
}

fn normalized_numbers_from_fields(
    raw_number: Option<&str>,
    raw_item_type: Option<&str>,
    raw_number_list: Option<&str>,
) -> Vec<String> {
    let mut numbers = Vec::new();

    if let Some(number) = normalize_number(raw_number, raw_item_type, raw_number_list) {
        if !numbers.iter().any(|existing| existing == &number) {
            numbers.push(number);
        }
    }

    if let Some(raw_number_list) = raw_number_list {
        if let Ok(list) = serde_json::from_str::<Vec<String>>(raw_number_list) {
            for item in list {
                let number = digits_only(&item);
                if !number.is_empty() && !numbers.iter().any(|existing| existing == &number) {
                    numbers.push(number);
                }
            }
        }
    }

    numbers
}

fn duplicate_merged_ids(mut models: Vec<bill_merged::Model>) -> Vec<i64> {
    if models.len() <= 1 {
        return Vec::new();
    }
    models.sort_by_key(merged_keep_rank);
    models.pop();
    models.into_iter().map(|model| model.id).collect()
}

fn duplicate_original_ids(mut models: Vec<bill_original::Model>) -> Vec<i64> {
    if models.len() <= 1 {
        return Vec::new();
    }
    models.sort_by_key(original_keep_rank);
    models.pop();
    models.into_iter().map(|model| model.id).collect()
}

fn merged_keep_rank(model: &bill_merged::Model) -> (i32, i32, i64) {
    let completeness = option_filled_score(&model.number)
        + option_filled_score(&model.position)
        + option_filled_score(&model.room)
        + option_filled_score(&model.notes)
        + option_filled_score(&model.target_user)
        + option_filled_score(&model.item_type)
        + option_filled_score(&model.synced_at)
        + option_filled_score(&model.source_account_id);
    let note_bonus = if model.notes.as_deref().map(str::trim).filter(|v| !v.is_empty()).is_some() {
        1
    } else {
        0
    };
    (completeness, note_bonus, model.id)
}

fn original_keep_rank(model: &bill_original::Model) -> (i32, i64) {
    let completeness = option_filled_score(&model.number)
        + option_filled_score(&model.target_user)
        + option_filled_score(&model.item_type)
        + option_filled_score(&model.synced_at);
    (completeness, model.id)
}

fn option_filled_score(value: &Option<String>) -> i32 {
    if value.as_deref().map(str::trim).filter(|v| !v.is_empty()).is_some() {
        1
    } else {
        0
    }
}
