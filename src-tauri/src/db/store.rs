use std::collections::HashSet;

use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, Set,
};
use shmtu_cas::classifier::PositionTranslator;
use shmtu_cas::datatype::bill::BillItem;
use tokio::sync::Mutex;

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
    known_numbers: Mutex<HashSet<String>>,
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
            known_numbers: Mutex::new(known_numbers),
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
            if let Some(ref nl) = row.number_list {
                if let Ok(list) = serde_json::from_str::<Vec<String>>(nl) {
                    for n in list {
                        numbers.insert(n);
                    }
                }
            }
        }

        let rows = bill_merged::Entity::find()
            .filter(bill_merged::Column::IdentityId.eq(identity_id))
            .all(db)
            .await?;
        for row in rows {
            if let Some(ref nl) = row.number_list {
                if let Ok(list) = serde_json::from_str::<Vec<String>>(nl) {
                    for n in list {
                        numbers.insert(n);
                    }
                }
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

        // 自动翻译 target_user → (position, room)
        let (position, room) = self
            .translator
            .translate(&bill.target_user)
            .unwrap_or_else(|| {
                // 模糊匹配：尝试用 target_user 整体作为关键词
                self.translator.translate_or_raw(&bill.target_user)
            });

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
            position: Set(None),
            room: Set(None),
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
        self.known_numbers.blocking_lock().contains(number)
    }

    fn merge(&mut self, new_bills: Vec<BillItem>) {
        if new_bills.is_empty() {
            return;
        }

        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let handle = tokio::runtime::Handle::current();

        for bill in &new_bills {
            for n in &bill.number_list {
                self.known_numbers.blocking_lock().insert(n.clone());
            }

            let _ = handle.block_on(self.save_bill_original(bill, &now));
            let _ = handle.block_on(self.append_to_merged(bill, &now));
        }
    }
}
