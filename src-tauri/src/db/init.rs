use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::crypto::CryptoService;
use crate::error::{AppError, AppResult};
use crate::models::{Account, CreateAccountParams, CreateIdentityParams, Identity, SessionInfo};

/// 数据库管理器，负责数据库初始化、目录管理和基础 CRUD 操作
#[derive(Clone)]
pub struct DatabaseManager {
    data_dir: PathBuf,
}

impl DatabaseManager {
    /// 创建内部引用（Clone 简写）
    pub fn clone_ref(&self) -> Self {
        self.clone()
    }

    /// 创建数据库管理器，指定数据根目录
    pub fn new(data_dir: impl AsRef<Path>) -> Self {
        Self {
            data_dir: data_dir.as_ref().to_path_buf(),
        }
    }

    /// 使用默认数据目录
    pub fn default_data_dir() -> PathBuf {
        PathBuf::from("Data")
    }

    /// 获取数据根目录
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// 初始化所有数据目录和数据库
    pub fn initialize(&self) -> AppResult<()> {
        self.ensure_directories()?;
        self.init_global_db()?;
        Ok(())
    }

    /// 确保所有必要目录存在
    fn ensure_directories(&self) -> AppResult<()> {
        let dirs = [
            self.data_dir.clone(),
            self.identity_dir(),
            self.account_dir(),
            self.snapshot_dir(),
            self.models_dir(),
            self.export_dir(),
        ];
        for dir in dirs {
            std::fs::create_dir_all(&dir)
                .map_err(|e| AppError::Config(format!("创建目录 {:?} 失败: {}", dir, e)))?;
        }
        Ok(())
    }

    // === 路径访问 ===

    pub fn global_db_path(&self) -> PathBuf {
        self.data_dir.join("shmtu.terminal.sqlite")
    }

    pub fn session_db_path(&self, account_id: &str) -> PathBuf {
        self.account_dir().join(format!("{}_session.sqlite", account_id))
    }

    pub fn identity_dir(&self) -> PathBuf {
        self.data_dir.join("identity")
    }

    pub fn identity_db_path(&self, identity_id: i64) -> PathBuf {
        self.identity_dir().join(format!("{}.sqlite", identity_id))
    }

    pub fn account_dir(&self) -> PathBuf {
        self.data_dir.join("account")
    }

    pub fn account_db_path(&self, account_id: &str) -> PathBuf {
        self.account_dir().join(format!("{}.sqlite", account_id))
    }

    pub fn snapshot_dir(&self) -> PathBuf {
        self.data_dir.join("snapshot")
    }

    pub fn models_dir(&self) -> PathBuf {
        self.data_dir.join("models")
    }

    pub fn export_dir(&self) -> PathBuf {
        self.data_dir.join("export")
    }

    // === 全局数据库操作 ===

    /// 打开全局数据库连接
    pub fn open_global_db(&self) -> AppResult<Connection> {
        let path = self.global_db_path();
        let conn = Connection::open(&path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        Ok(conn)
    }

    /// 初始化全局数据库表
    fn init_global_db(&self) -> AppResult<()> {
        let conn = self.open_global_db()?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS identities (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                name            TEXT NOT NULL,
                enable          INTEGER NOT NULL DEFAULT 1,
                enable_update   INTEGER NOT NULL DEFAULT 1,
                birthday        TEXT,
                default_remember INTEGER NOT NULL DEFAULT 0,
                created_at      TEXT NOT NULL,
                updated_at      TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS accounts (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                identity_id     INTEGER NOT NULL,
                account_name    TEXT NOT NULL,
                account_id      TEXT NOT NULL UNIQUE,
                password        TEXT NOT NULL,
                enable          INTEGER NOT NULL DEFAULT 1,
                enable_update   INTEGER NOT NULL DEFAULT 1,
                expire_date     TEXT NOT NULL DEFAULT '2099-12-31',
                last_update_time TEXT NOT NULL DEFAULT '',
                created_at      TEXT NOT NULL,
                updated_at      TEXT NOT NULL,
                FOREIGN KEY (identity_id) REFERENCES identities(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_accounts_identity_id ON accounts(identity_id);
            CREATE INDEX IF NOT EXISTS idx_accounts_account_id ON accounts(account_id);",
        )?;

        Ok(())
    }

    // === 身份 CRUD ===

    /// 获取所有身份
    pub fn list_identities(&self) -> AppResult<Vec<Identity>> {
        let conn = self.open_global_db()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, enable, enable_update, birthday, default_remember, created_at, updated_at
             FROM identities ORDER BY id",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(Identity {
                id: row.get(0)?,
                name: row.get(1)?,
                enable: row.get::<_, i32>(2)? != 0,
                enable_update: row.get::<_, i32>(3)? != 0,
                birthday: row.get(4)?,
                default_remember: row.get::<_, i32>(5)? != 0,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })?;

        let mut identities = Vec::new();
        for row in rows {
            identities.push(row?);
        }
        Ok(identities)
    }

    /// 创建身份（检查名称重复）
    pub fn create_identity(&self, params: &CreateIdentityParams) -> AppResult<i64> {
        let conn = self.open_global_db()?;

        let mut stmt = conn.prepare("SELECT COUNT(*) FROM identities WHERE name=?1")?;
        let count: i32 = stmt.query_row([&params.name], |row| row.get(0))?;
        if count > 0 {
            return Err(AppError::Validation(format!(
                "身份名称 '{}' 已存在",
                params.name
            )));
        }

        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO identities (name, enable, enable_update, birthday, default_remember, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            (
                &params.name,
                params.enable.unwrap_or(true) as i32,
                params.enable_update.unwrap_or(true) as i32,
                &params.birthday,
                params.default_remember.unwrap_or(false) as i32,
                &now,
                &now,
            ),
        )?;

        let id = conn.last_insert_rowid();
        self.init_identity_db(id)?;
        Ok(id)
    }

    /// 更新身份
    pub fn update_identity(&self, identity: &Identity) -> AppResult<()> {
        let conn = self.open_global_db()?;
        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE identities SET name=?1, enable=?2, enable_update=?3, birthday=?4,
             default_remember=?5, updated_at=?6 WHERE id=?7",
            (
                &identity.name,
                identity.enable as i32,
                identity.enable_update as i32,
                &identity.birthday,
                identity.default_remember as i32,
                &now,
                identity.id,
            ),
        )?;

        Ok(())
    }

    /// 删除身份（同时删除身份数据库和其下所有账号数据库）
    pub fn delete_identity(&self, identity_id: i64) -> AppResult<()> {
        let accounts = self.list_accounts_by_identity(identity_id)?;

        let conn = self.open_global_db()?;
        conn.execute("DELETE FROM identities WHERE id=?1", [identity_id])?;

        for account in accounts {
            let db_path = self.account_db_path(&account.account_id);
            if db_path.exists() {
                if let Err(e) = std::fs::remove_file(&db_path) {
                    eprintln!("删除账号数据库失败: {}", e);
                }
            }
        }

        let identity_db_path = self.identity_db_path(identity_id);
        if identity_db_path.exists() {
            if let Err(e) = std::fs::remove_file(&identity_db_path) {
                eprintln!("删除身份数据库失败: {}", e);
            }
        }

        Ok(())
    }

    /// 获取身份
    pub fn get_identity(&self, identity_id: i64) -> AppResult<Option<Identity>> {
        let conn = self.open_global_db()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, enable, enable_update, birthday, default_remember, created_at, updated_at
             FROM identities WHERE id=?1",
        )?;

        let mut rows = stmt.query_map([identity_id], |row| {
            Ok(Identity {
                id: row.get(0)?,
                name: row.get(1)?,
                enable: row.get::<_, i32>(2)? != 0,
                enable_update: row.get::<_, i32>(3)? != 0,
                birthday: row.get(4)?,
                default_remember: row.get::<_, i32>(5)? != 0,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })?;

        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    // === 账号 CRUD ===

    /// 获取所有账号
    pub fn list_all_accounts(&self) -> AppResult<Vec<Account>> {
        let conn = self.open_global_db()?;
        let mut stmt = conn.prepare(
            "SELECT id, identity_id, account_name, account_id, password, enable, enable_update,
             expire_date, last_update_time, created_at, updated_at
             FROM accounts ORDER BY id",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(Account {
                id: row.get(0)?,
                identity_id: row.get(1)?,
                account_name: row.get(2)?,
                account_id: row.get(3)?,
                password: row.get(4)?,
                enable: row.get::<_, i32>(5)? != 0,
                enable_update: row.get::<_, i32>(6)? != 0,
                expire_date: row.get(7)?,
                last_update_time: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        })?;

        let mut accounts = Vec::new();
        for row in rows {
            accounts.push(row?);
        }
        Ok(accounts)
    }

    /// 获取身份下所有账号
    pub fn list_accounts_by_identity(&self, identity_id: i64) -> AppResult<Vec<Account>> {
        let conn = self.open_global_db()?;
        let mut stmt = conn.prepare(
            "SELECT id, identity_id, account_name, account_id, password, enable, enable_update,
             expire_date, last_update_time, created_at, updated_at
             FROM accounts WHERE identity_id=?1 ORDER BY id",
        )?;

        let rows = stmt.query_map([identity_id], |row| {
            Ok(Account {
                id: row.get(0)?,
                identity_id: row.get(1)?,
                account_name: row.get(2)?,
                account_id: row.get(3)?,
                password: row.get(4)?,
                enable: row.get::<_, i32>(5)? != 0,
                enable_update: row.get::<_, i32>(6)? != 0,
                expire_date: row.get(7)?,
                last_update_time: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        })?;

        let mut accounts = Vec::new();
        for row in rows {
            accounts.push(row?);
        }
        Ok(accounts)
    }

    /// 创建账号（密码加密存储）
    pub fn create_account(
        &self,
        params: &CreateAccountParams,
        crypto: &CryptoService,
    ) -> AppResult<i64> {
        if params.account_id.len() != 12 || !params.account_id.chars().all(|c| c.is_ascii_digit()) {
            return Err(AppError::Validation("学号必须为12位数字".to_string()));
        }
        if params.password.is_empty() {
            return Err(AppError::Validation("密码不能为空".to_string()));
        }

        let encrypted_password = crypto.encrypt_string(&params.password)?;
        let now = chrono::Utc::now().to_rfc3339();

        let conn = self.open_global_db()?;
        conn.execute(
            "INSERT INTO accounts (identity_id, account_name, account_id, password, enable, enable_update,
             expire_date, last_update_time, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            (
                params.identity_id,
                &params.account_name,
                &params.account_id,
                &encrypted_password,
                params.enable.unwrap_or(true) as i32,
                params.enable_update.unwrap_or(true) as i32,
                params.expire_date.as_deref().unwrap_or("2099-12-31"),
                "",
                &now,
                &now,
            ),
        )?;

        let id = conn.last_insert_rowid();
        self.init_account_db(&params.account_id)?;
        Ok(id)
    }

    /// 获取账号
    pub fn get_account(&self, account_id: i64) -> AppResult<Option<Account>> {
        let conn = self.open_global_db()?;
        let mut stmt = conn.prepare(
            "SELECT id, identity_id, account_name, account_id, password, enable, enable_update,
             expire_date, last_update_time, created_at, updated_at
             FROM accounts WHERE id=?1",
        )?;

        let mut rows = stmt.query_map([account_id], |row| {
            Ok(Account {
                id: row.get(0)?,
                identity_id: row.get(1)?,
                account_name: row.get(2)?,
                account_id: row.get(3)?,
                password: row.get(4)?,
                enable: row.get::<_, i32>(5)? != 0,
                enable_update: row.get::<_, i32>(6)? != 0,
                expire_date: row.get(7)?,
                last_update_time: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        })?;

        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    /// 通过学号获取账号
    pub fn get_account_by_student_id(&self, student_id: &str) -> AppResult<Option<Account>> {
        let conn = self.open_global_db()?;
        let mut stmt = conn.prepare(
            "SELECT id, identity_id, account_name, account_id, password, enable, enable_update,
             expire_date, last_update_time, created_at, updated_at
             FROM accounts WHERE account_id=?1",
        )?;

        let mut rows = stmt.query_map([student_id], |row| {
            Ok(Account {
                id: row.get(0)?,
                identity_id: row.get(1)?,
                account_name: row.get(2)?,
                account_id: row.get(3)?,
                password: row.get(4)?,
                enable: row.get::<_, i32>(5)? != 0,
                enable_update: row.get::<_, i32>(6)? != 0,
                expire_date: row.get(7)?,
                last_update_time: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        })?;

        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    /// 更新账号（密码应由调用方预先加密）
    pub fn update_account(&self, account: &Account, crypto: &CryptoService) -> AppResult<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.open_global_db()?;

        // 空密码表示不修改密码，只更新其他字段
        if account.password.is_empty() {
            conn.execute(
                "UPDATE accounts SET account_name=?1, enable=?2, enable_update=?3, expire_date=?4, last_update_time=?5, updated_at=?6 WHERE id=?7",
                (
                    &account.account_name,
                    account.enable as i32,
                    account.enable_update as i32,
                    &account.expire_date,
                    &account.last_update_time,
                    &now,
                    account.id,
                ),
            )?;
        } else {
            // 有新密码，加密后更新
            let encrypted = crypto.encrypt_string(&account.password)?;
            conn.execute(
                "UPDATE accounts SET account_name=?1, password=?2, enable=?3, enable_update=?4, expire_date=?5, last_update_time=?6, updated_at=?7 WHERE id=?8",
                (
                    &account.account_name,
                    &encrypted,
                    account.enable as i32,
                    account.enable_update as i32,
                    &account.expire_date,
                    &account.last_update_time,
                    &now,
                    account.id,
                ),
            )?;
        }

        Ok(())
    }

    /// 更新账号密码
    pub fn update_account_password(
        &self,
        account_id: i64,
        new_password: &str,
        crypto: &CryptoService,
    ) -> AppResult<()> {
        if new_password.is_empty() {
            return Err(AppError::Validation("密码不能为空".to_string()));
        }
        let encrypted = crypto.encrypt_string(new_password)?;
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.open_global_db()?;
        conn.execute(
            "UPDATE accounts SET password=?1, updated_at=?2 WHERE id=?3",
            (&encrypted, &now, account_id),
        )?;
        Ok(())
    }

    /// 解密账号密码
    pub fn decrypt_account_password(
        &self,
        account: &Account,
        crypto: &CryptoService,
    ) -> AppResult<String> {
        crypto.decrypt_string(&account.password)
    }

    /// 删除账号
    pub fn delete_account(&self, account_id: i64) -> AppResult<()> {
        if let Some(account) = self.get_account(account_id)? {
            let db_path = self.account_db_path(&account.account_id);
            if db_path.exists() {
                if let Err(e) = std::fs::remove_file(&db_path) {
                    eprintln!("删除账号数据库失败: {}", e);
                }
            }
        }

        let conn = self.open_global_db()?;
        conn.execute("DELETE FROM accounts WHERE id=?1", [account_id])?;
        Ok(())
    }

    /// 更新账号最后同步时间
    pub fn update_account_last_sync(&self, account_id: i64) -> AppResult<()> {
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let conn = self.open_global_db()?;
        conn.execute(
            "UPDATE accounts SET last_update_time=?1, updated_at=?1 WHERE id=?2",
            (&now, account_id),
        )?;
        Ok(())
    }

    // === 身份数据库操作 ===

    /// 初始化身份数据库（合并账单 + 操作记录）
    fn init_identity_db(&self, identity_id: i64) -> AppResult<()> {
        let path = self.identity_db_path(identity_id);
        let conn = Connection::open(&path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS bill_merged (
                id                      INTEGER PRIMARY KEY AUTOINCREMENT,
                date_str                TEXT NOT NULL,
                time_str                TEXT NOT NULL,
                time_str_formatted      TEXT,
                date_time_formatted     TEXT,
                end_date_time_formatted TEXT,
                timestamp               INTEGER,
                end_timestamp           INTEGER,
                item_type               TEXT,
                number                  TEXT,
                number_list             TEXT,
                target_user             TEXT,
                money_str               TEXT,
                money                   REAL,
                method                  TEXT,
                status_str              TEXT,
                is_combined             INTEGER NOT NULL DEFAULT 0,
                source_account_id       TEXT,
                is_manual               INTEGER NOT NULL DEFAULT 0,
                synced_at               TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_bill_merged_timestamp ON bill_merged(timestamp);
            CREATE INDEX IF NOT EXISTS idx_bill_merged_number_list ON bill_merged(number_list);
            CREATE INDEX IF NOT EXISTS idx_bill_merged_source_account ON bill_merged(source_account_id);

            CREATE TABLE IF NOT EXISTS operation_log (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                operation_type  TEXT NOT NULL,
                record_numbers  TEXT,
                operation_time  TEXT NOT NULL,
                description     TEXT,
                account_id      TEXT
            );",
        )?;

        Ok(())
    }

    /// 打开身份数据库连接
    pub fn open_identity_db(&self, identity_id: i64) -> AppResult<Connection> {
        let path = self.identity_db_path(identity_id);
        if !path.exists() {
            self.init_identity_db(identity_id)?;
        }
        let conn = Connection::open(&path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        Ok(conn)
    }

    // === 账号数据库操作 ===

    /// 初始化账号原始数据库
    fn init_account_db(&self, account_id: &str) -> AppResult<()> {
        let path = self.account_db_path(account_id);
        let conn = Connection::open(&path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS bill_original (
                id                      INTEGER PRIMARY KEY AUTOINCREMENT,
                date_str                TEXT NOT NULL,
                time_str                TEXT NOT NULL,
                time_str_formatted      TEXT,
                date_time_formatted     TEXT,
                end_date_time_formatted TEXT,
                timestamp               INTEGER,
                end_timestamp           INTEGER,
                item_type               TEXT,
                number                  TEXT,
                number_list             TEXT,
                target_user             TEXT,
                money_str               TEXT,
                money                   REAL,
                method                  TEXT,
                status_str              TEXT,
                is_combined             INTEGER NOT NULL DEFAULT 0,
                account_id              TEXT NOT NULL,
                synced_at               TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_bill_original_timestamp ON bill_original(timestamp);
            CREATE INDEX IF NOT EXISTS idx_bill_original_number_list ON bill_original(number_list);
            CREATE INDEX IF NOT EXISTS idx_bill_original_account_id ON bill_original(account_id);",
        )?;

        Ok(())
    }

    /// 打开账号数据库连接
    pub fn open_account_db(&self, account_id: &str) -> AppResult<Connection> {
        let path = self.account_db_path(account_id);
        if !path.exists() {
            self.init_account_db(account_id)?;
        }
        let conn = Connection::open(&path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        Ok(conn)
    }

    // === 会话数据库操作 ===

    /// 初始化会话数据库
    fn init_session_db(&self, account_id: &str) -> AppResult<()> {
        let path = self.session_db_path(account_id);
        let conn = Connection::open(&path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS session_info (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                account_id  TEXT NOT NULL UNIQUE,
                cookies     TEXT NOT NULL,
                login_time  TEXT,
                expire_time TEXT,
                is_valid    INTEGER NOT NULL DEFAULT 1
            );

            CREATE INDEX IF NOT EXISTS idx_session_info_account_id ON session_info(account_id);",
        )?;

        Ok(())
    }

    /// 打开会话数据库连接
    pub fn open_session_db(&self, account_id: &str) -> AppResult<Connection> {
        let path = self.session_db_path(account_id);
        if !path.exists() {
            self.init_session_db(account_id)?;
        }
        let conn = Connection::open(&path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        Ok(conn)
    }

    // === 会话 CRUD ===

    /// 保存会话信息（加密 cookies）
    pub fn save_session(
        &self,
        account_id: &str,
        cookies: &str,
        crypto: &CryptoService,
    ) -> AppResult<()> {
        let encrypted_cookies = crypto.encrypt_string(cookies)?;
        let now = chrono::Local::now().to_rfc3339();
        let expire = chrono::Local::now() + chrono::Duration::minutes(30);

        let conn = self.open_session_db(account_id)?;
        conn.execute(
            "INSERT OR REPLACE INTO session_info (account_id, cookies, login_time, expire_time, is_valid)
             VALUES (?1, ?2, ?3, ?4, 1)",
            (account_id, &encrypted_cookies, &now, expire.to_rfc3339()),
        )?;
        Ok(())
    }

    /// 获取会话信息
    pub fn get_session(
        &self,
        account_id: &str,
        crypto: &CryptoService,
    ) -> AppResult<Option<SessionInfo>> {
        let conn = self.open_session_db(account_id)?;
        let mut stmt = conn.prepare(
            "SELECT id, account_id, cookies, login_time, expire_time, is_valid
             FROM session_info WHERE account_id=?1 AND is_valid=1",
        )?;

        let mut rows = stmt.query_map([account_id], |row| {
            Ok(SessionInfo {
                id: row.get(0)?,
                account_id: row.get(1)?,
                cookies: row.get(2)?,
                login_time: row.get(3)?,
                expire_time: row.get(4)?,
                is_valid: row.get::<_, i32>(5)? != 0,
            })
        })?;

        match rows.next() {
            Some(row) => {
                let mut session = row?;
                let decrypted = crypto.decrypt_string(&session.cookies)?;
                session.cookies = decrypted;
                Ok(Some(session))
            }
            None => Ok(None),
        }
    }

    /// 使会话失效
    pub fn invalidate_session(&self, account_id: &str) -> AppResult<()> {
        let conn = self.open_session_db(account_id)?;
        conn.execute(
            "UPDATE session_info SET is_valid=0 WHERE account_id=?1",
            [account_id],
        )?;
        Ok(())
    }

    /// 删除会话
    pub fn delete_session(&self, account_id: &str) -> AppResult<()> {
        let conn = self.open_session_db(account_id)?;
        conn.execute("DELETE FROM session_info WHERE account_id=?1", [account_id])?;
        Ok(())
    }

    // === 数据清理操作（全量更新用） ===

    /// 清空账号原始数据库
    pub fn clear_account_original(&self, account_id: &str) -> AppResult<()> {
        let conn = self.open_account_db(account_id)?;
        conn.execute("DELETE FROM bill_original", [])?;
        Ok(())
    }

    /// 清空身份合并数据库中指定账号来源的记录（保留手动添加的记录）
    pub fn clear_merged_by_account(&self, identity_id: i64, account_id: &str) -> AppResult<()> {
        let conn = self.open_identity_db(identity_id)?;
        conn.execute(
            "DELETE FROM bill_merged WHERE source_account_id=?1 AND is_manual=0",
            [account_id],
        )?;
        Ok(())
    }

    /// 清空身份合并数据库中的非手动记录
    pub fn clear_merged_non_manual(&self, identity_id: i64) -> AppResult<()> {
        let conn = self.open_identity_db(identity_id)?;
        conn.execute("DELETE FROM bill_merged WHERE is_manual=0", [])?;
        Ok(())
    }

    /// 清空操作日志
    pub fn clear_operation_logs(
        &self,
        identity_id: i64,
        account_id: Option<&str>,
    ) -> AppResult<()> {
        let conn = self.open_identity_db(identity_id)?;
        match account_id {
            Some(aid) => {
                conn.execute("DELETE FROM operation_log WHERE account_id=?1", [aid])?;
            }
            None => {
                conn.execute("DELETE FROM operation_log", [])?;
            }
        }
        Ok(())
    }
}
