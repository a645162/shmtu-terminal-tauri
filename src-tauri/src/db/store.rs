use std::collections::HashSet;

use rusqlite::Connection;
use shmtu_cas::datatype::bill::BillItem;

use crate::db::DatabaseManager;
use crate::error::AppResult;
use crate::models::{BillMerged, BillOriginal, OperationLog};

/// 实现 shmtu-cas-rs 的 BillStore trait，用于增量同步
pub struct BillStoreImpl {
    db_manager: DatabaseManager,
    /// 当前账号的学号
    account_id: String,
    /// 当前身份 ID
    identity_id: i64,
    /// 已缓存的交易号集合，用于快速去重
    known_numbers: HashSet<String>,
}

impl BillStoreImpl {
    /// 创建账号级别的 BillStore
    pub fn new(db_manager: DatabaseManager, account_id: &str, identity_id: i64) -> AppResult<Self> {
        let known_numbers = Self::load_known_numbers(&db_manager, account_id, identity_id)?;
        Ok(Self {
            db_manager,
            account_id: account_id.to_string(),
            identity_id,
            known_numbers,
        })
    }

    /// 从数据库加载已存在的交易号
    fn load_known_numbers(
        db_manager: &DatabaseManager,
        account_id: &str,
        identity_id: i64,
    ) -> AppResult<HashSet<String>> {
        let mut numbers = HashSet::new();

        // 从账号原始数据库加载
        if let Ok(conn) = db_manager.open_account_db(account_id) {
            let mut stmt = conn.prepare("SELECT number_list FROM bill_original")?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
            for row in rows {
                if let Ok(json_str) = row {
                    if let Ok(list) = serde_json::from_str::<Vec<String>>(&json_str) {
                        for n in list {
                            numbers.insert(n);
                        }
                    }
                }
            }
        }

        // 从身份数据库合并表加载
        if let Ok(conn) = db_manager.open_identity_db(identity_id) {
            let mut stmt = conn.prepare("SELECT number_list FROM bill_merged")?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
            for row in rows {
                if let Ok(json_str) = row {
                    if let Ok(list) = serde_json::from_str::<Vec<String>>(&json_str) {
                        for n in list {
                            numbers.insert(n);
                        }
                    }
                }
            }
        }

        Ok(numbers)
    }

    /// 将 BillItem 写入账号原始数据库
    pub fn save_bill_original(&self, conn: &Connection, bill: &BillItem, now: &str) -> AppResult<()> {
        let number_list_json = serde_json::to_string(&bill.number_list)?;
        conn.execute(
            "INSERT INTO bill_original (
                date_str, time_str, time_str_formatted, date_time_formatted,
                end_date_time_formatted, timestamp, end_timestamp, item_type,
                number, number_list, target_user, money_str, money, method,
                status_str, is_combined, account_id, synced_at
            ) VALUES (:date_str, :time_str, :time_str_formatted, :date_time_formatted,
                :end_date_time_formatted, :timestamp, :end_timestamp, :item_type,
                :number, :number_list, :target_user, :money_str, :money, :method,
                :status_str, :is_combined, :account_id, :synced_at)",
            rusqlite::named_params! {
                ":date_str": bill.date_str,
                ":time_str": bill.time_str,
                ":time_str_formatted": bill.time_str_formatted,
                ":date_time_formatted": bill.date_time_formatted,
                ":end_date_time_formatted": bill.end_date_time_formatted,
                ":timestamp": bill.timestamp,
                ":end_timestamp": bill.end_timestamp,
                ":item_type": bill.item_type,
                ":number": bill.number,
                ":number_list": number_list_json,
                ":target_user": bill.target_user,
                ":money_str": bill.money_str,
                ":money": bill.money as f64,
                ":method": bill.method,
                ":status_str": bill.status_str,
                ":is_combined": bill.is_combined as i32,
                ":account_id": self.account_id,
                ":synced_at": now,
            },
        )?;
        Ok(())
    }

    /// 将 BillItem 追加到身份合并数据库
    pub fn append_to_merged(&self, conn: &Connection, bill: &BillItem, now: &str) -> AppResult<()> {
        let number_list_json = serde_json::to_string(&bill.number_list)?;
        conn.execute(
            "INSERT INTO bill_merged (
                date_str, time_str, time_str_formatted, date_time_formatted,
                end_date_time_formatted, timestamp, end_timestamp, item_type,
                number, number_list, target_user, money_str, money, method,
                status_str, is_combined, source_account_id, is_manual, synced_at
            ) VALUES (:date_str, :time_str, :time_str_formatted, :date_time_formatted,
                :end_date_time_formatted, :timestamp, :end_timestamp, :item_type,
                :number, :number_list, :target_user, :money_str, :money, :method,
                :status_str, :is_combined, :source_account_id, 0, :synced_at)",
            rusqlite::named_params! {
                ":date_str": bill.date_str,
                ":time_str": bill.time_str,
                ":time_str_formatted": bill.time_str_formatted,
                ":date_time_formatted": bill.date_time_formatted,
                ":end_date_time_formatted": bill.end_date_time_formatted,
                ":timestamp": bill.timestamp,
                ":end_timestamp": bill.end_timestamp,
                ":item_type": bill.item_type,
                ":number": bill.number,
                ":number_list": number_list_json,
                ":target_user": bill.target_user,
                ":money_str": bill.money_str,
                ":money": bill.money as f64,
                ":method": bill.method,
                ":status_str": bill.status_str,
                ":is_combined": bill.is_combined as i32,
                ":source_account_id": self.account_id,
                ":synced_at": now,
            },
        )?;
        Ok(())
    }

    /// 获取账号原始账单列表（分页）
    pub fn list_original_bills(
        &self,
        account_id: &str,
        page: u32,
        page_size: u32,
    ) -> AppResult<(Vec<BillOriginal>, u32)> {
        let conn = self.db_manager.open_account_db(account_id)?;
        let offset = (page - 1) * page_size;

        // 获取总数
        let total: u32 = conn.query_row(
            "SELECT COUNT(*) FROM bill_original", [],
            |row| row.get(0),
        )?;

        let total_pages = (total + page_size - 1) / page_size;

        let mut stmt = conn.prepare(
            "SELECT id, date_str, time_str, time_str_formatted, date_time_formatted,
             end_date_time_formatted, timestamp, end_timestamp, item_type, number,
             number_list, target_user, money_str, money, method, status_str,
             is_combined, account_id, synced_at
             FROM bill_original ORDER BY timestamp DESC LIMIT ?1 OFFSET ?2",
        )?;

        let rows = stmt.query_map((page_size, offset), |row| {
            Ok(BillOriginal {
                id: row.get(0)?,
                date_str: row.get(1)?,
                time_str: row.get(2)?,
                time_str_formatted: row.get(3)?,
                date_time_formatted: row.get(4)?,
                end_date_time_formatted: row.get(5)?,
                timestamp: row.get(6)?,
                end_timestamp: row.get(7)?,
                item_type: row.get(8)?,
                number: row.get(9)?,
                number_list: row.get(10)?,
                target_user: row.get(11)?,
                money_str: row.get(12)?,
                money: row.get(13)?,
                method: row.get(14)?,
                status_str: row.get(15)?,
                is_combined: row.get::<_, i32>(16)? != 0,
                account_id: row.get(17)?,
                synced_at: row.get(18)?,
            })
        })?;

        let mut bills = Vec::new();
        for row in rows {
            bills.push(row?);
        }

        Ok((bills, total_pages))
    }

    /// 获取身份合并账单列表（分页）
    pub fn list_merged_bills(
        &self,
        identity_id: i64,
        page: u32,
        page_size: u32,
    ) -> AppResult<(Vec<BillMerged>, u32)> {
        let conn = self.db_manager.open_identity_db(identity_id)?;
        let offset = (page - 1) * page_size;

        let total: u32 = conn.query_row(
            "SELECT COUNT(*) FROM bill_merged", [],
            |row| row.get(0),
        )?;

        let total_pages = if page_size > 0 { (total + page_size - 1) / page_size } else { 1 };

        let mut stmt = conn.prepare(
            "SELECT id, date_str, time_str, time_str_formatted, date_time_formatted,
             end_date_time_formatted, timestamp, end_timestamp, item_type, number,
             number_list, target_user, money_str, money, method, status_str,
             is_combined, source_account_id, is_manual, synced_at
             FROM bill_merged ORDER BY timestamp DESC LIMIT ?1 OFFSET ?2",
        )?;

        let rows = stmt.query_map((page_size, offset), |row| {
            Ok(BillMerged {
                id: row.get(0)?,
                date_str: row.get(1)?,
                time_str: row.get(2)?,
                time_str_formatted: row.get(3)?,
                date_time_formatted: row.get(4)?,
                end_date_time_formatted: row.get(5)?,
                timestamp: row.get(6)?,
                end_timestamp: row.get(7)?,
                item_type: row.get(8)?,
                number: row.get(9)?,
                number_list: row.get(10)?,
                target_user: row.get(11)?,
                money_str: row.get(12)?,
                money: row.get(13)?,
                method: row.get(14)?,
                status_str: row.get(15)?,
                is_combined: row.get::<_, i32>(16)? != 0,
                source_account_id: row.get(17)?,
                is_manual: row.get::<_, i32>(18)? != 0,
                synced_at: row.get(19)?,
            })
        })?;

        let mut bills = Vec::new();
        for row in rows {
            bills.push(row?);
        }

        Ok((bills, total_pages))
    }

    /// 手动添加合并账单记录
    pub fn add_manual_bill(&self, identity_id: i64, bill: &BillItem) -> AppResult<i64> {
        let conn = self.db_manager.open_identity_db(identity_id)?;
        let number_list_json = serde_json::to_string(&bill.number_list)?;
        let now = chrono::Local::now().to_rfc3339();

        conn.execute(
            "INSERT INTO bill_merged (
                date_str, time_str, time_str_formatted, date_time_formatted,
                end_date_time_formatted, timestamp, end_timestamp, item_type,
                number, number_list, target_user, money_str, money, method,
                status_str, is_combined, source_account_id, is_manual, synced_at
            ) VALUES (:date_str, :time_str, :time_str_formatted, :date_time_formatted,
                :end_date_time_formatted, :timestamp, :end_timestamp, :item_type,
                :number, :number_list, :target_user, :money_str, :money, :method,
                :status_str, :is_combined, NULL, 1, :synced_at)",
            rusqlite::named_params! {
                ":date_str": bill.date_str,
                ":time_str": bill.time_str,
                ":time_str_formatted": bill.time_str_formatted,
                ":date_time_formatted": bill.date_time_formatted,
                ":end_date_time_formatted": bill.end_date_time_formatted,
                ":timestamp": bill.timestamp,
                ":end_timestamp": bill.end_timestamp,
                ":item_type": bill.item_type,
                ":number": bill.number,
                ":number_list": number_list_json,
                ":target_user": bill.target_user,
                ":money_str": bill.money_str,
                ":money": bill.money as f64,
                ":method": bill.method,
                ":status_str": bill.status_str,
                ":is_combined": bill.is_combined as i32,
                ":synced_at": now,
            },
        )?;

        let id = conn.last_insert_rowid();

        // 记录操作日志
        self.log_operation(
            &conn,
            "add",
            &number_list_json,
            &format!("手动添加账单: {} {}", bill.date_time_formatted, bill.item_type),
            None,
        )?;

        Ok(id)
    }

    /// 手动删除合并账单记录
    pub fn delete_merged_bill(&self, identity_id: i64, bill_id: i64) -> AppResult<()> {
        let conn = self.db_manager.open_identity_db(identity_id)?;

        // 获取要删除的记录信息用于日志
        let number_list: Option<String> = conn.query_row(
            "SELECT number_list FROM bill_merged WHERE id=?1",
            [bill_id],
            |row| row.get(0),
        ).ok();

        conn.execute("DELETE FROM bill_merged WHERE id=?1", [bill_id])?;

        // 记录操作日志
        if let Some(nl) = number_list {
            self.log_operation(
                &conn,
                "delete",
                &nl,
                &format!("手动删除账单 ID={}", bill_id),
                None,
            )?;
        }

        Ok(())
    }

    /// 记录操作日志
    fn log_operation(
        &self,
        conn: &Connection,
        operation_type: &str,
        record_numbers: &str,
        description: &str,
        account_id: Option<&str>,
    ) -> AppResult<()> {
        let now = chrono::Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO operation_log (operation_type, record_numbers, operation_time, description, account_id)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            (operation_type, record_numbers, &now, description, account_id),
        )?;
        Ok(())
    }

    /// 获取操作日志列表
    pub fn list_operation_logs(&self, identity_id: i64) -> AppResult<Vec<OperationLog>> {
        let conn = self.db_manager.open_identity_db(identity_id)?;
        let mut stmt = conn.prepare(
            "SELECT id, operation_type, record_numbers, operation_time, description, account_id
             FROM operation_log ORDER BY id DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(OperationLog {
                id: row.get(0)?,
                operation_type: row.get(1)?,
                record_numbers: row.get(2)?,
                operation_time: row.get(3)?,
                description: row.get(4)?,
                account_id: row.get(5)?,
            })
        })?;

        let mut logs = Vec::new();
        for row in rows {
            logs.push(row?);
        }
        Ok(logs)
    }

    /// 获取所有合并账单（用于导出）
    pub fn get_all_merged_bills(&self, identity_id: i64) -> AppResult<Vec<BillMerged>> {
        let conn = self.db_manager.open_identity_db(identity_id)?;
        let mut stmt = conn.prepare(
            "SELECT id, date_str, time_str, time_str_formatted, date_time_formatted,
             end_date_time_formatted, timestamp, end_timestamp, item_type, number,
             number_list, target_user, money_str, money, method, status_str,
             is_combined, source_account_id, is_manual, synced_at
             FROM bill_merged ORDER BY timestamp ASC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(BillMerged {
                id: row.get(0)?,
                date_str: row.get(1)?,
                time_str: row.get(2)?,
                time_str_formatted: row.get(3)?,
                date_time_formatted: row.get(4)?,
                end_date_time_formatted: row.get(5)?,
                timestamp: row.get(6)?,
                end_timestamp: row.get(7)?,
                item_type: row.get(8)?,
                number: row.get(9)?,
                number_list: row.get(10)?,
                target_user: row.get(11)?,
                money_str: row.get(12)?,
                money: row.get(13)?,
                method: row.get(14)?,
                status_str: row.get(15)?,
                is_combined: row.get::<_, i32>(16)? != 0,
                source_account_id: row.get(17)?,
                is_manual: row.get::<_, i32>(18)? != 0,
                synced_at: row.get(19)?,
            })
        })?;

        let mut bills = Vec::new();
        for row in rows {
            bills.push(row?);
        }
        Ok(bills)
    }

    /// 获取所有原始账单（用于导出）
    pub fn get_all_original_bills(&self, account_id: &str) -> AppResult<Vec<BillOriginal>> {
        let conn = self.db_manager.open_account_db(account_id)?;
        let mut stmt = conn.prepare(
            "SELECT id, date_str, time_str, time_str_formatted, date_time_formatted,
             end_date_time_formatted, timestamp, end_timestamp, item_type, number,
             number_list, target_user, money_str, money, method, status_str,
             is_combined, account_id, synced_at
             FROM bill_original ORDER BY timestamp ASC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(BillOriginal {
                id: row.get(0)?,
                date_str: row.get(1)?,
                time_str: row.get(2)?,
                time_str_formatted: row.get(3)?,
                date_time_formatted: row.get(4)?,
                end_date_time_formatted: row.get(5)?,
                timestamp: row.get(6)?,
                end_timestamp: row.get(7)?,
                item_type: row.get(8)?,
                number: row.get(9)?,
                number_list: row.get(10)?,
                target_user: row.get(11)?,
                money_str: row.get(12)?,
                money: row.get(13)?,
                method: row.get(14)?,
                status_str: row.get(15)?,
                is_combined: row.get::<_, i32>(16)? != 0,
                account_id: row.get(17)?,
                synced_at: row.get(18)?,
            })
        })?;

        let mut bills = Vec::new();
        for row in rows {
            bills.push(row?);
        }
        Ok(bills)
    }
}

/// 实现 shmtu-cas-rs 的 BillStore trait
impl shmtu_cas::sync::BillStore for BillStoreImpl {
    fn contains(&self, number: &str) -> bool {
        self.known_numbers.contains(number)
    }

    fn merge(&mut self, new_bills: Vec<BillItem>) {
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

        // 打开两个数据库连接
        let account_conn = match self.db_manager.open_account_db(&self.account_id) {
            Ok(c) => c,
            Err(_) => return,
        };
        let identity_conn = match self.db_manager.open_identity_db(self.identity_id) {
            Ok(c) => c,
            Err(_) => return,
        };

        for bill in &new_bills {
            // 更新已知交易号集合
            for n in &bill.number_list {
                self.known_numbers.insert(n.clone());
            }

            // 写入账号原始数据库
            if let Err(e) = self.save_bill_original(&account_conn, bill, &now) {
                eprintln!("写入原始账单失败: {}", e);
            }

            // 追加到身份合并数据库
            if let Err(e) = self.append_to_merged(&identity_conn, bill, &now) {
                eprintln!("追加合并账单失败: {}", e);
            }
        }
    }
}
