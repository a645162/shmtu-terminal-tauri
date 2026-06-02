use std::collections::{HashMap, HashSet};

use crate::db::init::{
    bill_merged_model_to_app, bill_original_model_to_app, operation_log_model_to_app,
};
use crate::entity::*;
use crate::error::AppResult;
use crate::models::{BillMerged, BillOriginal, OperationLog};
use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};
use shmtu_cas::classifier::PositionTranslator;
use shmtu_cas::datatype::bill::BillItem;

/// 账单存储实现，负责原始账单和合并账单的持久化、去重与规范化。
///
/// 每个 `BillStoreImpl` 绑定一个账号（account_id）和身份（identity_id），
/// 维护一个 `known_numbers` 集合用于增量同步时快速判断账单是否已存在，
/// 以及一个 `pending_bills` 缓冲区用于批量写入。
pub struct BillStoreImpl {
    db: DatabaseConnection,
    account_id: String,
    identity_id: i64,
    /// 已知交易号的纯数字集合，用于增量同步去重
    known_numbers: HashSet<String>,
    /// 待刷入数据库的新账单缓冲区
    pending_bills: Vec<BillItem>,
    /// 位置翻译器，将对方账户名解析为校区/房间号
    translator: PositionTranslator,
}

impl BillStoreImpl {
    /// 创建新的 `BillStoreImpl` 实例。
    ///
    /// 从数据库中加载该账号+身份下所有已有的交易号到 `known_numbers`，
    /// 后续调用 `contains()` 即可在内存中快速判断账单是否已存在。
    pub async fn new(
        db: DatabaseConnection,
        account_id: &str,
        identity_id: i64,
        translator: PositionTranslator,
    ) -> AppResult<Self> {
        tracing::info!(
            "[Store] BillStoreImpl::new account_id={}, identity_id={}",
            account_id,
            identity_id
        );

        let known_numbers = Self::load_known_numbers(&db, account_id, identity_id).await?;

        tracing::info!(
            "[Store] BillStoreImpl loaded {} known numbers for account={}, identity={}",
            known_numbers.len(),
            account_id,
            identity_id
        );

        Ok(Self {
            db,
            account_id: account_id.to_string(),
            identity_id,
            known_numbers,
            pending_bills: Vec::new(),
            translator,
        })
    }

    /// 从原始账单表和合并账单表中加载所有已有的规范化交易号。
    ///
    /// 同时查询两张表是因为同一笔交易可能同时出现在原始和合并数据中，
    /// 需要合并去重以确保 `known_numbers` 完整。
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
        tracing::debug!(
            "[Store] load_known_numbers: {} original rows for account={}",
            rows.len(),
            account_id
        );
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
        tracing::debug!(
            "[Store] load_known_numbers: {} merged rows for identity={}",
            rows.len(),
            identity_id
        );
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

    /// 将一条原始账单写入 bill_original 表。
    ///
    /// 仅写入原始数据，不做任何规范化或去重判断。
    /// 调用方应先通过 `BillStore::contains()` 确认账单不重复。
    pub async fn save_bill_original(&self, bill: &BillItem, now: &str) -> AppResult<()> {
        tracing::debug!(
            "[Store] save_bill_original: date={}, type={}, number={}",
            bill.date_str,
            bill.item_type,
            bill.number
        );

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

    /// 重建指定身份的合并账单表。
    ///
    /// 清空该身份下所有合并账单，再从原始账单表逐条重新写入合并表。
    /// 返回重建的记录数。
    pub async fn rebuild_merged_from_original(&self, identity_id: i64) -> AppResult<usize> {
        tracing::info!(
            "[Store] rebuild_merged_from_original: identity_id={}",
            identity_id
        );

        // 清空该身份的合并表
        bill_merged::Entity::delete_many()
            .filter(bill_merged::Column::IdentityId.eq(identity_id))
            .exec(&self.db)
            .await?;

        // 从原始表读取所有属于该身份的账号的账单
        let accounts = accounts::Entity::find()
            .filter(accounts::Column::IdentityId.eq(identity_id))
            .all(&self.db)
            .await?;

        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let mut count = 0usize;

        for acct in &accounts {
            let originals = bill_original::Entity::find()
                .filter(bill_original::Column::AccountId.eq(&acct.account_id))
                .order_by_asc(bill_original::Column::Timestamp)
                .all(&self.db)
                .await?;

            for orig in originals {
                let (position, room) =
                    self.resolve_position_and_room(&orig.target_user.clone().unwrap_or_default());

                let model = bill_merged::ActiveModel {
                    identity_id: Set(identity_id),
                    date_str: Set(orig.date_str.clone()),
                    time_str: Set(orig.time_str.clone()),
                    time_str_formatted: Set(orig.time_str_formatted.clone()),
                    date_time_formatted: Set(orig.date_time_formatted.clone()),
                    end_date_time_formatted: Set(orig.end_date_time_formatted.clone()),
                    timestamp: Set(orig.timestamp),
                    end_timestamp: Set(orig.end_timestamp),
                    item_type: Set(orig.item_type.clone()),
                    number: Set(orig.number.clone()),
                    number_list: Set(orig.number_list.clone()),
                    target_user: Set(orig.target_user.clone()),
                    money_str: Set(orig.money_str.clone()),
                    money: Set(orig.money),
                    method: Set(orig.method.clone()),
                    status_str: Set(orig.status_str.clone()),
                    is_combined: Set(true),
                    source_account_id: Set(Some(acct.account_id.clone())),
                    is_manual: Set(false),
                    position: Set(Some(position)),
                    room: Set(Some(room)),
                    notes: Set(None),
                    synced_at: Set(Some(now.clone())),
                    ..Default::default()
                };
                bill_merged::Entity::insert(model).exec(&self.db).await?;
                count += 1;
            }
        }

        tracing::info!(
            "[Store] rebuild_merged_from_original: identity_id={}, rebuilt {} records",
            identity_id,
            count
        );

        Ok(count)
    }

    /// 将一条账单追加到 bill_merged 合并表。
    ///
    /// 同时通过 `PositionTranslator` 解析对方账户对应的校区和房间号，
    /// 写入 position/room 字段以便后续按位置筛选。
    pub async fn append_to_merged(&self, bill: &BillItem, now: &str) -> AppResult<()> {
        let number_list_json = serde_json::to_string(&bill.number_list)?;

        let (position, room) = self.resolve_position_and_room(&bill.target_user);

        tracing::debug!(
            "[Store] append_to_merged: target_user={} -> position={}, room={}",
            bill.target_user,
            position,
            room
        );

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

    /// 将对方账户名解析为（校区, 房间号）。
    ///
    /// 优先使用精确翻译，若无法精确匹配则回退到模糊匹配返回原始字符串。
    fn resolve_position_and_room(&self, target_user: &str) -> (String, String) {
        self.translator
            .translate(target_user)
            .unwrap_or_else(|| self.translator.translate_or_raw(target_user))
    }

    /// 回填合并账单表中缺失或格式不规范的元数据。
    ///
    /// 筛选条件：number 为空、position/room 为空、或 item_type 包含"交易号"（旧格式数据）。
    /// 对每条记录执行规范化后逐条更新，仅更新确实发生变化的字段。
    /// 返回实际更新的记录数。
    pub async fn backfill_merged_metadata(&self, identity_id: i64) -> AppResult<usize> {
        use sea_orm::{ActiveModelTrait, IntoActiveModel};

        tracing::info!(
            "[Store] backfill_merged_metadata: identity_id={}",
            identity_id
        );

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

        tracing::debug!(
            "[Store] backfill_merged_metadata: {} candidates need checking",
            candidates.len()
        );

        let mut updated = 0usize;

        for model in candidates {
            let normalized_item_type = model
                .item_type
                .as_deref()
                .map(normalize_item_type)
                .filter(|s| !s.is_empty());
            let normalized_number = normalize_number(
                model.number.as_deref(),
                model.item_type.as_deref(),
                model.number_list.as_deref(),
            );
            let normalized_number_list = normalize_number_list_json(
                model.number_list.as_deref(),
                normalized_number.as_deref(),
            );
            let normalized_target_user = model
                .target_user
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            let normalized_position_room = normalized_target_user
                .as_deref()
                .map(|target_user| self.resolve_position_and_room(target_user));

            let needs_update = normalized_item_type != model.item_type
                || normalized_number != model.number
                || normalized_number_list != model.number_list
                || normalized_target_user != model.target_user
                || normalized_position_room.as_ref().map(|(p, _)| p) != model.position.as_ref()
                || normalized_position_room.as_ref().map(|(_, r)| r) != model.room.as_ref();

            if !needs_update {
                continue;
            }

            tracing::debug!(
                "[Store] backfill_merged_metadata: updating id={}, number {:?} -> {:?}, position {:?} -> {:?}",
                model.id,
                model.number,
                normalized_number,
                model.position,
                normalized_position_room.as_ref().map(|(p, _)| p),
            );

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

        tracing::info!(
            "[Store] backfill_merged_metadata: updated {} of {} candidates",
            updated,
            updated
        );
        Ok(updated)
    }

    /// 回填原始账单表中缺失或格式不规范的元数据。
    ///
    /// 与 `backfill_merged_metadata` 类似，但仅处理原始账单表，
    /// 且不涉及 position/room 字段（原始表无此字段）。
    pub async fn backfill_original_metadata(&self, account_id: &str) -> AppResult<usize> {
        use sea_orm::{ActiveModelTrait, IntoActiveModel};

        tracing::info!(
            "[Store] backfill_original_metadata: account_id={}",
            account_id
        );

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

        tracing::debug!(
            "[Store] backfill_original_metadata: {} candidates need checking",
            candidates.len()
        );

        let mut updated = 0usize;

        for model in candidates {
            let normalized_item_type = model
                .item_type
                .as_deref()
                .map(normalize_item_type)
                .filter(|s| !s.is_empty());
            let normalized_number = normalize_number(
                model.number.as_deref(),
                model.item_type.as_deref(),
                model.number_list.as_deref(),
            );
            let normalized_number_list = normalize_number_list_json(
                model.number_list.as_deref(),
                normalized_number.as_deref(),
            );

            let needs_update = normalized_item_type != model.item_type
                || normalized_number != model.number
                || normalized_number_list != model.number_list;

            if !needs_update {
                continue;
            }

            tracing::debug!(
                "[Store] backfill_original_metadata: updating id={}, number {:?} -> {:?}",
                model.id,
                model.number,
                normalized_number,
            );

            let mut active = model.into_active_model();
            active.item_type = Set(normalized_item_type);
            active.number = Set(normalized_number);
            active.number_list = Set(normalized_number_list);
            active.update(&self.db).await?;
            updated += 1;
        }

        tracing::info!(
            "[Store] backfill_original_metadata: updated {} records for account={}",
            updated,
            account_id
        );
        Ok(updated)
    }

    /// 对指定身份下的合并账单进行去重。
    ///
    /// 按交易号（纯数字）分组，每组保留 `merged_keep_rank` 最高的记录，
    /// 其余全部删除。返回被删除的记录数。
    pub async fn dedupe_merged_by_identity(&self, identity_id: i64) -> AppResult<usize> {
        tracing::info!(
            "[Store] dedupe_merged_by_identity: identity_id={}",
            identity_id
        );

        let models = bill_merged::Entity::find()
            .filter(bill_merged::Column::IdentityId.eq(identity_id))
            .order_by_asc(bill_merged::Column::Id)
            .all(&self.db)
            .await?;

        let mut grouped: HashMap<String, Vec<bill_merged::Model>> = HashMap::new();
        for model in models {
            let Some(number) = model
                .number
                .as_deref()
                .map(digits_only)
                .filter(|n| !n.is_empty())
            else {
                continue;
            };
            grouped.entry(number).or_default().push(model);
        }

        tracing::debug!(
            "[Store] dedupe_merged: {} unique numbers, {} groups have duplicates",
            grouped.len(),
            grouped.values().filter(|g| g.len() > 1).count()
        );

        let duplicate_ids = grouped
            .into_values()
            .flat_map(|models| duplicate_merged_ids(models))
            .collect::<Vec<_>>();

        if duplicate_ids.is_empty() {
            tracing::info!("[Store] dedupe_merged: no duplicates found");
            return Ok(0);
        }

        tracing::info!(
            "[Store] dedupe_merged: deleting {} duplicate records",
            duplicate_ids.len()
        );

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

    /// 对指定账号下的原始账单进行去重。
    ///
    /// 逻辑与 `dedupe_merged_by_identity` 相同，但操作原始账单表，
    /// 使用 `original_keep_rank` 作为保留优先级。
    pub async fn dedupe_original_by_account(&self, account_id: &str) -> AppResult<usize> {
        tracing::info!(
            "[Store] dedupe_original_by_account: account_id={}",
            account_id
        );

        let models = bill_original::Entity::find()
            .filter(bill_original::Column::AccountId.eq(account_id))
            .order_by_asc(bill_original::Column::Id)
            .all(&self.db)
            .await?;

        let mut grouped: HashMap<String, Vec<bill_original::Model>> = HashMap::new();
        for model in models {
            let Some(number) = model
                .number
                .as_deref()
                .map(digits_only)
                .filter(|n| !n.is_empty())
            else {
                continue;
            };
            grouped.entry(number).or_default().push(model);
        }

        tracing::debug!(
            "[Store] dedupe_original: {} unique numbers, {} groups have duplicates",
            grouped.len(),
            grouped.values().filter(|g| g.len() > 1).count()
        );

        let duplicate_ids = grouped
            .into_values()
            .flat_map(|models| duplicate_original_ids(models))
            .collect::<Vec<_>>();

        if duplicate_ids.is_empty() {
            tracing::info!("[Store] dedupe_original: no duplicates found");
            return Ok(0);
        }

        tracing::info!(
            "[Store] dedupe_original: deleting {} duplicate records",
            duplicate_ids.len()
        );

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

    /// 将缓冲区中的待写入账单批量刷入数据库。
    ///
    /// 每条账单同时写入原始表和合并表。刷入后清空缓冲区。
    pub async fn flush_pending_bills(&mut self) -> AppResult<()> {
        if self.pending_bills.is_empty() {
            return Ok(());
        }

        tracing::info!(
            "[Store] flush_pending_bills: flushing {} bills",
            self.pending_bills.len()
        );

        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let pending_bills = std::mem::take(&mut self.pending_bills);

        for bill in &pending_bills {
            self.save_bill_original(bill, &now).await?;
            self.append_to_merged(bill, &now).await?;
        }

        tracing::info!(
            "[Store] flush_pending_bills: {} bills flushed",
            pending_bills.len()
        );
        Ok(())
    }

    /// 分页查询指定账号的原始账单。
    ///
    /// 按时间戳降序排列，返回账单列表和总页数。
    pub async fn list_original_bills(
        &self,
        account_id: &str,
        page: u32,
        page_size: u32,
    ) -> AppResult<(Vec<BillOriginal>, u32)> {
        tracing::debug!(
            "[Store] list_original_bills: account={}, page={}, page_size={}",
            account_id,
            page,
            page_size
        );

        let paginator = bill_original::Entity::find()
            .filter(bill_original::Column::AccountId.eq(account_id))
            .order_by_desc(bill_original::Column::Timestamp)
            .paginate(&self.db, page_size as u64);

        let total = paginator.num_items().await? as u32;
        let total_pages = total.div_ceil(page_size);

        let models = paginator.fetch_page(page as u64).await?;
        let bills: Vec<BillOriginal> = models.into_iter().map(bill_original_model_to_app).collect();

        tracing::debug!(
            "[Store] list_original_bills: returned {} bills, total_pages={}",
            bills.len(),
            total_pages
        );

        Ok((bills, total_pages))
    }

    /// 分页查询指定身份的合并账单。
    ///
    /// 按时间戳降序排列，返回账单列表和总页数。
    pub async fn list_merged_bills(
        &self,
        identity_id: i64,
        page: u32,
        page_size: u32,
    ) -> AppResult<(Vec<BillMerged>, u32)> {
        tracing::debug!(
            "[Store] list_merged_bills: identity={}, page={}, page_size={}",
            identity_id,
            page,
            page_size
        );

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
        let bills: Vec<BillMerged> = models.into_iter().map(bill_merged_model_to_app).collect();

        tracing::debug!(
            "[Store] list_merged_bills: returned {} bills, total_pages={}",
            bills.len(),
            total_pages
        );

        Ok((bills, total_pages))
    }

    /// 手动添加一条账单到合并表。
    ///
    /// 与同步写入不同，手动添加的账单标记 `is_manual=true`，
    /// 清空同步时不会被自动删除，且不关联 source_account_id。
    /// 返回新插入记录的 ID。
    pub async fn add_manual_bill(&self, identity_id: i64, bill: &BillItem) -> AppResult<i64> {
        tracing::info!(
            "[Store] add_manual_bill: identity={}, type={}, money={}",
            identity_id,
            bill.item_type,
            bill.money_str
        );

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

        tracing::info!("[Store] add_manual_bill: inserted id={}", id);

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

    /// 更新指定账单的备注字段。
    pub async fn update_bill_notes(&self, bill_id: i64, notes: Option<String>) -> AppResult<()> {
        tracing::debug!(
            "[Store] update_bill_notes: bill_id={}, notes={:?}",
            bill_id,
            notes
                .as_deref()
                .map(|n| if n.len() > 20 { &n[..20] } else { n })
        );

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

    /// 删除指定合并账单，并记录操作日志。
    pub async fn delete_merged_bill(&self, _identity_id: i64, bill_id: i64) -> AppResult<()> {
        tracing::info!("[Store] delete_merged_bill: bill_id={}", bill_id);

        let number_list = bill_merged::Entity::find_by_id(bill_id)
            .one(&self.db)
            .await?
            .and_then(|m| m.number_list);

        bill_merged::Entity::delete_by_id(bill_id)
            .exec(&self.db)
            .await?;

        if let Some(nl) = number_list {
            self.log_operation("delete", &nl, &format!("手动删除账单 ID={}", bill_id), None)
                .await?;
        }

        Ok(())
    }

    /// 写入一条操作日志到 operation_log 表。
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

    /// 查询指定身份的所有操作日志，按 ID 降序排列。
    pub async fn list_operation_logs(&self, identity_id: i64) -> AppResult<Vec<OperationLog>> {
        tracing::debug!("[Store] list_operation_logs: identity_id={}", identity_id);

        let models = operation_log::Entity::find()
            .filter(operation_log::Column::IdentityId.eq(identity_id))
            .order_by_desc(operation_log::Column::Id)
            .all(&self.db)
            .await?;

        Ok(models.into_iter().map(operation_log_model_to_app).collect())
    }

    /// 获取指定身份下所有合并账单（不分页），按时间升序。
    ///
    /// 主要用于导出操作，数据量可能较大。
    pub async fn get_all_merged_bills(&self, identity_id: i64) -> AppResult<Vec<BillMerged>> {
        tracing::debug!("[Store] get_all_merged_bills: identity_id={}", identity_id);

        let models = bill_merged::Entity::find()
            .filter(bill_merged::Column::IdentityId.eq(identity_id))
            .order_by_asc(bill_merged::Column::Timestamp)
            .all(&self.db)
            .await?;

        tracing::debug!(
            "[Store] get_all_merged_bills: {} bills loaded",
            models.len()
        );

        Ok(models.into_iter().map(bill_merged_model_to_app).collect())
    }

    /// 获取指定账号下所有原始账单（不分页），按时间升序。
    ///
    /// 主要用于导出操作，数据量可能较大。
    pub async fn get_all_original_bills(&self, account_id: &str) -> AppResult<Vec<BillOriginal>> {
        tracing::debug!("[Store] get_all_original_bills: account_id={}", account_id);

        let models = bill_original::Entity::find()
            .filter(bill_original::Column::AccountId.eq(account_id))
            .order_by_asc(bill_original::Column::Timestamp)
            .all(&self.db)
            .await?;

        tracing::debug!(
            "[Store] get_all_original_bills: {} bills loaded",
            models.len()
        );

        Ok(models.into_iter().map(bill_original_model_to_app).collect())
    }
}

/// `BillStore` trait 的实现，供同步流程调用。
///
/// `contains()` 用于增量同步时判断某笔账单是否已存在；
/// `merge()` 接收新账单列表，过滤掉已存在的后暂存到缓冲区。
impl shmtu_cas::sync::BillStore for BillStoreImpl {
    fn contains(&self, number: &str) -> bool {
        self.known_numbers.contains(number)
    }

    fn merge(&mut self, new_bills: Vec<BillItem>) {
        if new_bills.is_empty() {
            return;
        }

        let mut deduped_bills = Vec::with_capacity(new_bills.len());
        let mut duplicate_count = 0usize;

        for bill in new_bills {
            let numbers = candidate_bill_numbers(&bill);
            // 任意一个交易号命中已知集合，即视为重复
            if !numbers.is_empty()
                && numbers
                    .iter()
                    .any(|number| self.known_numbers.contains(number))
            {
                duplicate_count += 1;
                continue;
            }

            for number in &numbers {
                self.known_numbers.insert(number.clone());
            }
            deduped_bills.push(bill);
        }

        if duplicate_count > 0 {
            tracing::debug!(
                "[Store] merge: {} duplicates filtered out, {} new bills kept",
                duplicate_count,
                deduped_bills.len()
            );
        }

        tracing::debug!(
            "[Store] merge: {} bills added to pending (total pending: {})",
            deduped_bills.len(),
            self.pending_bills.len() + deduped_bills.len()
        );

        self.pending_bills.extend(deduped_bills);
    }
}

/// 规范化 item_type 字段：去除"交易号"后缀及其前的冒号。
///
/// 旧数据中 item_type 可能包含"交易号：123456"这样的混合格式，
/// 本函数将"交易号"及其后面的内容移除，只保留交易类型名称。
fn normalize_item_type(raw: &str) -> String {
    let compact = compact_whitespace(raw);
    let before_marker = compact.split("交易号").next().unwrap_or(&compact).trim();
    before_marker
        .trim_end_matches([':', '：'])
        .trim()
        .to_string()
}

/// 规范化交易号：从多个可能来源中提取纯数字交易号。
///
/// 优先级：
/// 1. `raw_number` 字段中的纯数字
/// 2. `raw_item_type` 中"交易号"标记后的数字
/// 3. `raw_number_list` JSON 数组中第一个非空纯数字
fn normalize_number(
    raw_number: Option<&str>,
    raw_item_type: Option<&str>,
    raw_number_list: Option<&str>,
) -> Option<String> {
    let number = digits_only(raw_number.unwrap_or_default());
    if !number.is_empty() {
        return Some(number);
    }

    // 回退：从 item_type 中提取"交易号"后的数字
    if let Some(number) = raw_item_type.and_then(extract_transaction_number) {
        return Some(number);
    }

    // 最后回退：从 number_list JSON 中取第一个
    raw_number_list
        .and_then(first_number_from_number_list)
        .filter(|number| !number.is_empty())
}

/// 规范化 number_list JSON 字段：去重并确保纯数字格式。
///
/// 先解析现有 JSON 数组中的数字（去除非数字字符），
/// 再将 `number` 参数（来自 `normalize_number` 的结果）追加到列表末尾（若不存在）。
/// 返回去重后的 JSON 数组字符串。
fn normalize_number_list_json(
    raw_number_list: Option<&str>,
    number: Option<&str>,
) -> Option<String> {
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

/// 从 number_list JSON 字符串中提取第一个非空纯数字交易号。
fn first_number_from_number_list(raw_number_list: &str) -> Option<String> {
    serde_json::from_str::<Vec<String>>(raw_number_list)
        .ok()?
        .into_iter()
        .map(|item| digits_only(&item))
        .find(|item| !item.is_empty())
}

/// 从包含"交易号"标记的字符串中提取交易号数字。
///
/// 例如 "充值交易号123456" -> Some("123456")。
/// 若标记后无数字则返回 None。
fn extract_transaction_number(raw: &str) -> Option<String> {
    let marker_index = raw.find("交易号")?;
    let number = digits_only(&raw[marker_index..]);
    if number.is_empty() {
        tracing::warn!(
            "[Store] extract_transaction_number: found marker but no digits in '{}'",
            raw
        );
        None
    } else {
        Some(number)
    }
}

/// 提取字符串中所有 ASCII 数字字符。
fn digits_only(raw: &str) -> String {
    raw.chars().filter(|ch| ch.is_ascii_digit()).collect()
}

/// 将连续空白压缩为单个空格。
fn compact_whitespace(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 从一笔账单中提取所有候选交易号（纯数字），用于去重判断。
///
/// 合并 number_list 和 number 字段中的数字，去重后返回。
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

/// 从原始字段中提取所有规范化的交易号（去重）。
///
/// 用于初始化 `known_numbers` 集合，同时覆盖 number 字段和 number_list 字段。
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

/// 从一组相同交易号的合并账单中，选出应保留的记录，返回其余应删除的 ID 列表。
///
/// 保留策略：按 `merged_keep_rank` 排序后弹出得分最高的（保留），
/// 其余全部标记为重复删除。
fn duplicate_merged_ids(mut models: Vec<bill_merged::Model>) -> Vec<i64> {
    if models.len() <= 1 {
        return Vec::new();
    }
    tracing::debug!(
        "[Store] duplicate_merged_ids: {} records with same number",
        models.len()
    );
    models.sort_by_key(merged_keep_rank);
    // 排序后最后一个得分最高，弹出保留
    models.pop();
    models.into_iter().map(|model| model.id).collect()
}

/// 从一组相同交易号的原始账单中，选出应保留的记录，返回其余应删除的 ID 列表。
///
/// 逻辑与 `duplicate_merged_ids` 相同，使用 `original_keep_rank` 排序。
fn duplicate_original_ids(mut models: Vec<bill_original::Model>) -> Vec<i64> {
    if models.len() <= 1 {
        return Vec::new();
    }
    tracing::debug!(
        "[Store] duplicate_original_ids: {} records with same number",
        models.len()
    );
    models.sort_by_key(original_keep_rank);
    models.pop();
    models.into_iter().map(|model| model.id).collect()
}

/// 计算合并账单的保留优先级排名。
///
/// 排名元组：(字段完整度得分, 备注填充加分, 记录ID)。
/// - 字段完整度：每有一个非空字段得 1 分（共 8 个可填字段）
/// - 备注加分：有非空备注额外加 1，因为手动添加的备注不应丢失
/// - ID：同分时保留较早的记录（ID 更小优先）
///
/// 排序后元组值最大的记录被保留。
fn merged_keep_rank(model: &bill_merged::Model) -> (i32, i32, i64) {
    let completeness = option_filled_score(&model.number)
        + option_filled_score(&model.position)
        + option_filled_score(&model.room)
        + option_filled_score(&model.notes)
        + option_filled_score(&model.target_user)
        + option_filled_score(&model.item_type)
        + option_filled_score(&model.synced_at)
        + option_filled_score(&model.source_account_id);
    let note_bonus = if model
        .notes
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .is_some()
    {
        1
    } else {
        0
    };
    (completeness, note_bonus, model.id)
}

/// 计算原始账单的保留优先级排名。
///
/// 排名元组：(字段完整度得分, 记录ID)。
/// 字段完整度基于 4 个可填字段。同分时保留 ID 较小的记录。
fn original_keep_rank(model: &bill_original::Model) -> (i32, i64) {
    let completeness = option_filled_score(&model.number)
        + option_filled_score(&model.target_user)
        + option_filled_score(&model.item_type)
        + option_filled_score(&model.synced_at);
    (completeness, model.id)
}

/// 判断 Option<String> 字段是否有实质内容（非空且非纯空白）。
///
/// 有内容返回 1，无内容返回 0。用于计算记录的完整度得分。
fn option_filled_score(value: &Option<String>) -> i32 {
    if value
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .is_some()
    {
        1
    } else {
        0
    }
}
