//! 账单合并服务 (Rust 端)
//!
//! 对应 Android 端 `BillMergeService`：
//! - 相邻洗澡/热水账单 < 阈值（默认 15 分钟，可配置）时首尾合并
//! - 合并后存到 `bill_merged` 表，原始账单保留在 `bill_original`
//! - 实时合并：新增账单时立即检查

use crate::entity::bill_merged;
use chrono::Utc;
use sea_orm::entity::prelude::*;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, Set};
use std::collections::HashSet;

const BATH_KEYWORDS: &[&str] = &[
    "浴", "洗澡", "热水", "淋浴", "水控", "洗浴", "bath", "shower", "wash",
];

pub const DEFAULT_MERGE_THRESHOLD_MINUTES: i64 = 15;

#[derive(Debug, Clone)]
pub struct MergeConfig {
    pub threshold_minutes: i64,
}

impl Default for MergeConfig {
    fn default() -> Self {
        Self {
            threshold_minutes: DEFAULT_MERGE_THRESHOLD_MINUTES,
        }
    }
}

pub struct BillMergeService {
    db: DatabaseConnection,
    config: MergeConfig,
}

impl BillMergeService {
    pub fn new(db: DatabaseConnection, config: MergeConfig) -> Self {
        Self { db, config }
    }

    pub fn with_default_config(db: DatabaseConnection) -> Self {
        Self::new(db, MergeConfig::default())
    }

    pub fn is_bath_bill(item_type: &str, target_user: &str) -> bool {
        let type_lower = item_type.to_lowercase();
        let target_lower = target_user.to_lowercase();
        BATH_KEYWORDS
            .iter()
            .any(|kw| type_lower.contains(kw) || target_lower.contains(kw))
    }

    pub async fn find_mergeable_bill(
        &self,
        new_bill: &bill_merged::Model,
    ) -> Result<Option<bill_merged::Model>, DbErr> {
        if !Self::is_bath_bill(
            new_bill.item_type.as_deref().unwrap_or(""),
            new_bill.target_user.as_deref().unwrap_or(""),
        ) {
            return Ok(None);
        }

        let new_time = new_bill.timestamp.unwrap_or(0);
        let window_start = new_time - (self.config.threshold_minutes + 1) * 60;
        let window_end = new_time + 60;

        let candidates = bill_merged::Entity::find()
            .filter(bill_merged::Column::IdentityId.eq(new_bill.identity_id))
            .filter(bill_merged::Column::ItemType.eq(new_bill.item_type.clone()))
            .filter(bill_merged::Column::Timestamp.gte(window_start))
            .filter(bill_merged::Column::Timestamp.lte(window_end))
            .all(&self.db)
            .await?;

        let normalized_new = Self::normalize_target(&new_bill.target_user.clone().unwrap_or_default());
        let threshold_secs = self.config.threshold_minutes * 60;

        Ok(candidates
            .into_iter()
            .filter(|c| {
                c.status_str == new_bill.status_str
                    && Self::normalize_target(&c.target_user.clone().unwrap_or_default()) == normalized_new
            })
            .filter_map(|c| {
                let ct = c.timestamp.unwrap_or(0);
                let gap = new_time - ct;
                if (0..=threshold_secs).contains(&gap) { Some((c, gap)) } else { None }
            })
            .min_by_key(|(_, gap)| *gap)
            .map(|(c, _)| c))
    }

    pub fn merge_bills(
        &self,
        existing: &bill_merged::Model,
        new_bill: &bill_merged::Model,
    ) -> bill_merged::ActiveModel {
        let split_list = |s: &Option<String>| -> Vec<String> {
            s.as_deref()
                .unwrap_or("")
                .split('')
                .filter(|p| !p.is_empty())
                .map(String::from)
                .collect()
        };

        let existing_txns = if existing.is_combined {
            split_list(&existing.number_list)
        } else {
            vec![existing.number.clone().unwrap_or_default()]
        };
        let new_txns = if new_bill.is_combined {
            split_list(&new_bill.number_list)
        } else {
            vec![new_bill.number.clone().unwrap_or_default()]
        };
        let all_txns: Vec<String> = existing_txns
            .into_iter()
            .chain(new_txns.into_iter())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        let existing_times = if existing.is_combined {
            split_list(&existing.number_list)
        } else {
            vec![existing.date_time_formatted.clone().unwrap_or_default()]
        };
        let new_times = if new_bill.is_combined {
            split_list(&new_bill.number_list)
        } else {
            vec![new_bill.date_time_formatted.clone().unwrap_or_default()]
        };
        let mut all_times = existing_times;
        all_times.extend(new_times);
        all_times.sort();
        all_times.dedup();

        let total_money = existing.money.unwrap_or(0.0) + new_bill.money.unwrap_or(0.0);
        let end_time = all_times.last().cloned().unwrap_or_default();

        bill_merged::ActiveModel {
            id: Set(new_bill.id),
            identity_id: Set(new_bill.identity_id),
            date_str: Set(end_time.split(' ').next().unwrap_or("").to_string()),
            time_str: Set(end_time.split(' ').nth(1).unwrap_or("").to_string()),
            time_str_formatted: Set(Some(end_time.clone())),
            date_time_formatted: Set(Some(end_time.clone())),
            end_date_time_formatted: Set(Some(end_time.clone())),
            timestamp: Set(new_bill.timestamp),
            end_timestamp: Set(new_bill.timestamp),
            item_type: Set(new_bill.item_type.clone()),
            number: Set(new_bill.number.clone()),
            number_list: Set(Some(all_txns.join(""))),
            target_user: Set(new_bill.target_user.clone()),
            money_str: Set(Some(format!("{:.2}", total_money))),
            money: Set(Some(total_money)),
            method: Set(new_bill.method.clone()),
            status_str: Set(new_bill.status_str.clone()),
            is_combined: Set(true),
            source_account_id: Set(new_bill.source_account_id.clone()),
            is_manual: Set(new_bill.is_manual),
            position: Set(new_bill.position.clone()),
            room: Set(new_bill.room.clone()),
            category: Set(new_bill.category.clone()),
            notes: Set(new_bill.notes.clone()),
            synced_at: Set(Some(Utc::now().to_rfc3339())),
        }
    }

    fn normalize_target(s: &str) -> String {
        s.trim().split_whitespace().collect::<Vec<_>>().join("")
    }
}
