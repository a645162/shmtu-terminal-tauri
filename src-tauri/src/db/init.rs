use std::path::{Path, PathBuf};

use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection,
    EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, Set,
};

use crate::crypto::CryptoService;
use crate::entity::*;
use crate::error::{AppError, AppResult};
use crate::models::{
    Account, BillMerged, BillOriginal, CreateAccountParams, CreateIdentityParams, Identity,
    OperationLog, SessionInfo,
};

/// 数据库管理器，负责 SQLite 连接、表初始化/迁移以及全部 CRUD 操作。
///
/// 持有一个 `DatabaseConnection` 和数据目录路径，
/// 对外提供身份、账号、会话、账单数据清理等操作。
#[derive(Clone)]
pub struct DatabaseManager {
    db: DatabaseConnection,
    data_dir: PathBuf,
}

impl DatabaseManager {
    pub fn clone_ref(&self) -> Self {
        self.clone()
    }

    /// 连接到 SQLite 数据库并初始化表结构。
    ///
    /// 若数据目录不存在会自动创建，同时确保 identity/account/snapshot 等子目录存在。
    /// 数据库启用 WAL 模式和外键约束，然后执行建表和迁移 SQL。
    pub async fn connect(data_dir: impl AsRef<Path>) -> AppResult<Self> {
        let data_dir = data_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&data_dir)
            .map_err(|e| AppError::Config(format!("创建数据目录 {:?} 失败: {}", data_dir, e)))?;

        // Ensure subdirectories
        for sub in &["identity", "account", "snapshot", "models", "export"] {
            let dir = data_dir.join(sub);
            let _ = std::fs::create_dir_all(&dir);
        }

        let db_path = data_dir.join("shmtu.terminal.sqlite");
        let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());

        tracing::info!("[DB] Connecting to database: {}", db_path.display());

        let db = sea_orm::Database::connect(&db_url)
            .await
            .map_err(|e| AppError::Database(format!("数据库连接失败: {}", e)))?;

        tracing::info!("[DB] Database connected successfully");

        // Enable WAL mode
        db.execute(sea_orm::Statement::from_string(
            sea_orm::DatabaseBackend::Sqlite,
            "PRAGMA journal_mode=WAL;".to_owned(),
        ))
        .await
        .map_err(|e| AppError::Database(format!("设置WAL模式失败: {}", e)))?;

        db.execute(sea_orm::Statement::from_string(
            sea_orm::DatabaseBackend::Sqlite,
            "PRAGMA foreign_keys=ON;".to_owned(),
        ))
        .await
        .map_err(|e| AppError::Database(format!("设置外键失败: {}", e)))?;

        let manager = Self { db, data_dir };

        manager.init_tables().await?;

        Ok(manager)
    }

    /// 初始化表结构并执行数据库迁移。
    ///
    /// 使用 `CREATE TABLE IF NOT EXISTS` 保证幂等，
    /// ALTER TABLE 语句用 `let _` 忽略"列已存在"的错误，实现增量迁移。
    async fn init_tables(&self) -> AppResult<()> {
        use sea_orm::Statement;

        tracing::info!("[DB] Initializing tables");

        self.db
            .execute(Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                include_str!("../sql/create_tables.sql").to_string(),
            ))
            .await?;

        // Migration: add position/room columns to bill_merged (ignore if already exist)
        let _ = self
            .db
            .execute(Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                "ALTER TABLE bill_merged ADD COLUMN position TEXT DEFAULT NULL;".to_string(),
            ))
            .await;
        let _ = self
            .db
            .execute(Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                "ALTER TABLE bill_merged ADD COLUMN room TEXT DEFAULT NULL;".to_string(),
            ))
            .await;

        // Migration: add notes column to bill_merged (ignore if already exist)
        let _ = self
            .db
            .execute(Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                "ALTER TABLE bill_merged ADD COLUMN notes TEXT DEFAULT NULL;".to_string(),
            ))
            .await;

        // Migration: add category column to bill_merged (ignore if already exist)
        let _ = self
            .db
            .execute(Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                "ALTER TABLE bill_merged ADD COLUMN category TEXT DEFAULT NULL;".to_string(),
            ))
            .await;

        let _ = self
            .db
            .execute(Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                "ALTER TABLE accounts ADD COLUMN admission_date TEXT DEFAULT NULL;".to_string(),
            ))
            .await;
        let _ = self
            .db
            .execute(Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                "ALTER TABLE accounts ADD COLUMN graduation_date TEXT DEFAULT NULL;".to_string(),
            ))
            .await;
        let _ = self
            .db
            .execute(Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                "UPDATE accounts SET graduation_date = expire_date WHERE graduation_date IS NULL AND expire_date IS NOT NULL AND expire_date != '' AND expire_date != '2099-12-31';".to_string(),
            ))
            .await;

        tracing::info!("[DB] Tables initialized, migrations applied");
        Ok(())
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    pub fn snapshot_dir(&self) -> PathBuf {
        self.data_dir.join("snapshot")
    }

    pub fn identity_dir(&self) -> PathBuf {
        self.data_dir.join("identity")
    }

    pub fn account_dir(&self) -> PathBuf {
        self.data_dir.join("account")
    }

    pub fn identity_db_path(&self, identity_id: i64) -> PathBuf {
        self.identity_dir().join(format!("{}.sqlite", identity_id))
    }

    pub fn account_db_path(&self, account_id: &str) -> PathBuf {
        self.account_dir().join(format!("{}.sqlite", account_id))
    }

    pub fn session_db_path(&self, account_id: &str) -> PathBuf {
        self.account_dir()
            .join(format!("{}_session.sqlite", account_id))
    }
    // === 身份 CRUD ===

    /// 查询所有身份，按 ID 升序排列。
    pub async fn list_identities(&self) -> AppResult<Vec<Identity>> {
        tracing::debug!("[DB] list_identities");

        let models = identities::Entity::find()
            .order_by_asc(identities::Column::Id)
            .all(&self.db)
            .await?;

        tracing::debug!("[DB] list_identities: {} results", models.len());
        Ok(models.into_iter().map(identity_model_to_app).collect())
    }

    /// 创建新身份。若名称已存在则返回验证错误。
    pub async fn create_identity(&self, params: &CreateIdentityParams) -> AppResult<i64> {
        tracing::info!("[DB] create_identity: name={}", params.name);

        let count = identities::Entity::find()
            .filter(identities::Column::Name.eq(&params.name))
            .count(&self.db)
            .await?;
        if count > 0 {
            tracing::warn!(
                "[DB] create_identity: name '{}' already exists",
                params.name
            );
            return Err(AppError::Validation(format!(
                "身份名称 '{}' 已存在",
                params.name
            )));
        }

        let now = chrono::Utc::now().to_rfc3339();
        let model = identities::ActiveModel {
            name: Set(params.name.clone()),
            enable: Set(params.enable.unwrap_or(true)),
            enable_update: Set(params.enable_update.unwrap_or(true)),
            birthday: Set(params.birthday.clone()),
            default_remember: Set(params.default_remember.unwrap_or(false)),
            created_at: Set(now.clone()),
            updated_at: Set(now),
            ..Default::default()
        };

        let result = identities::Entity::insert(model).exec(&self.db).await?;
        tracing::info!("[DB] create_identity: created id={}", result.last_insert_id);
        Ok(result.last_insert_id)
    }

    /// 更新身份信息。
    pub async fn update_identity(&self, identity: &Identity) -> AppResult<()> {
        tracing::info!("[DB] update_identity: id={}", identity.id);

        let now = chrono::Utc::now().to_rfc3339();
        let model = identities::ActiveModel {
            id: Set(identity.id),
            name: Set(identity.name.clone()),
            enable: Set(identity.enable),
            enable_update: Set(identity.enable_update),
            birthday: Set(identity.birthday.clone()),
            default_remember: Set(identity.default_remember),
            updated_at: Set(now),
            ..Default::default()
        };
        model.update(&self.db).await?;
        Ok(())
    }

    /// 删除身份及其关联的所有数据。
    ///
    /// 级联删除：合并账单 -> 操作日志 -> 原始账单 -> 会话信息 -> 账号 -> 身份本身。
    pub async fn delete_identity(&self, identity_id: i64) -> AppResult<()> {
        tracing::warn!(
            "[DB] delete_identity: id={}, cascading delete all related data",
            identity_id
        );

        bill_merged::Entity::delete_many()
            .filter(bill_merged::Column::IdentityId.eq(identity_id))
            .exec(&self.db)
            .await?;
        operation_log::Entity::delete_many()
            .filter(operation_log::Column::IdentityId.eq(identity_id))
            .exec(&self.db)
            .await?;
        let accts = self.list_accounts_by_identity(identity_id).await?;
        for a in &accts {
            bill_original::Entity::delete_many()
                .filter(bill_original::Column::AccountId.eq(&a.account_id))
                .exec(&self.db)
                .await?;
            session_info::Entity::delete_many()
                .filter(session_info::Column::AccountId.eq(&a.account_id))
                .exec(&self.db)
                .await?;
        }
        accounts::Entity::delete_many()
            .filter(accounts::Column::IdentityId.eq(identity_id))
            .exec(&self.db)
            .await?;
        identities::Entity::delete_by_id(identity_id)
            .exec(&self.db)
            .await?;

        tracing::info!(
            "[DB] delete_identity: id={} deleted with {} accounts",
            identity_id,
            accts.len()
        );
        Ok(())
    }

    /// 根据 ID 查询单个身份。
    pub async fn get_identity(&self, identity_id: i64) -> AppResult<Option<Identity>> {
        tracing::debug!("[DB] get_identity: id={}", identity_id);

        let model = identities::Entity::find_by_id(identity_id)
            .one(&self.db)
            .await?;
        Ok(model.map(identity_model_to_app))
    }

    // === 账号 CRUD ===

    /// 查询所有账号，按 ID 升序排列。
    pub async fn list_all_accounts(&self) -> AppResult<Vec<Account>> {
        tracing::debug!("[DB] list_all_accounts");

        let models = accounts::Entity::find()
            .order_by_asc(accounts::Column::Id)
            .all(&self.db)
            .await?;
        Ok(models.into_iter().map(account_model_to_app).collect())
    }

    /// 查询指定身份下的所有账号，按 ID 升序排列。
    pub async fn list_accounts_by_identity(&self, identity_id: i64) -> AppResult<Vec<Account>> {
        tracing::debug!(
            "[DB] list_accounts_by_identity: identity_id={}",
            identity_id
        );

        let models = accounts::Entity::find()
            .filter(accounts::Column::IdentityId.eq(identity_id))
            .order_by_asc(accounts::Column::Id)
            .all(&self.db)
            .await?;
        Ok(models.into_iter().map(account_model_to_app).collect())
    }

    /// 创建新账号。学号必须为 12 位数字，密码经加密后存储。
    pub async fn create_account(
        &self,
        params: &CreateAccountParams,
        crypto: &CryptoService,
    ) -> AppResult<i64> {
        tracing::info!("[DB] create_account: student_id={}", params.account_id);

        if params.account_id.len() != 12 || !params.account_id.chars().all(|c| c.is_ascii_digit()) {
            tracing::warn!(
                "[DB] create_account: invalid student_id format '{}'",
                params.account_id
            );
            return Err(AppError::Validation("学号必须为12位数字".to_string()));
        }
        if params.password.is_empty() {
            tracing::warn!("[DB] create_account: empty password");
            return Err(AppError::Validation("密码不能为空".to_string()));
        }

        let encrypted_password = crypto.encrypt_string(&params.password)?;
        let now = chrono::Utc::now().to_rfc3339();

        let model = accounts::ActiveModel {
            identity_id: Set(params.identity_id),
            account_name: Set(params.account_name.clone()),
            account_id: Set(params.account_id.clone()),
            password: Set(encrypted_password),
            enable: Set(params.enable.unwrap_or(true)),
            enable_update: Set(params.enable_update.unwrap_or(true)),
            admission_date: Set(params.admission_date.clone()),
            graduation_date: Set(params.graduation_date.clone()),
            expire_date: Set(params
                .expire_date
                .as_deref()
                .unwrap_or("2099-12-31")
                .to_string()),
            last_update_time: Set(String::new()),
            created_at: Set(now.clone()),
            updated_at: Set(now),
            ..Default::default()
        };

        let result = accounts::Entity::insert(model).exec(&self.db).await?;
        tracing::info!("[DB] create_account: created id={}", result.last_insert_id);
        Ok(result.last_insert_id)
    }

    /// 根据数据库 ID 查询单个账号。
    pub async fn get_account(&self, account_id: i64) -> AppResult<Option<Account>> {
        tracing::debug!("[DB] get_account: id={}", account_id);

        let model = accounts::Entity::find_by_id(account_id)
            .one(&self.db)
            .await?;
        Ok(model.map(account_model_to_app))
    }

    /// 根据学号（account_id 字段）查询单个账号。
    pub async fn get_account_by_student_id(&self, student_id: &str) -> AppResult<Option<Account>> {
        tracing::debug!("[DB] get_account_by_student_id: student_id={}", student_id);

        let model = accounts::Entity::find()
            .filter(accounts::Column::AccountId.eq(student_id))
            .one(&self.db)
            .await?;
        Ok(model.map(account_model_to_app))
    }

    /// 更新账号信息。
    ///
    /// 若 `account.password` 为空则不更新密码字段（保留原值），
    /// 否则重新加密后更新。
    pub async fn update_account(&self, account: &Account, crypto: &CryptoService) -> AppResult<()> {
        tracing::info!("[DB] update_account: id={}", account.id);

        let now = chrono::Utc::now().to_rfc3339();

        if account.password.is_empty() {
            let model = accounts::ActiveModel {
                id: Set(account.id),
                account_name: Set(account.account_name.clone()),
                enable: Set(account.enable),
                enable_update: Set(account.enable_update),
                admission_date: Set(account.admission_date.clone()),
                graduation_date: Set(account.graduation_date.clone()),
                expire_date: Set(account.expire_date.clone()),
                last_update_time: Set(account.last_update_time.clone()),
                updated_at: Set(now),
                ..Default::default()
            };
            model.update(&self.db).await?;
        } else {
            let encrypted = crypto.encrypt_string(&account.password)?;
            let model = accounts::ActiveModel {
                id: Set(account.id),
                account_name: Set(account.account_name.clone()),
                password: Set(encrypted),
                enable: Set(account.enable),
                enable_update: Set(account.enable_update),
                admission_date: Set(account.admission_date.clone()),
                graduation_date: Set(account.graduation_date.clone()),
                expire_date: Set(account.expire_date.clone()),
                last_update_time: Set(account.last_update_time.clone()),
                updated_at: Set(now),
                ..Default::default()
            };
            model.update(&self.db).await?;
        }

        Ok(())
    }

    /// 单独更新账号密码（加密后存储）。
    pub async fn update_account_password(
        &self,
        account_id: i64,
        new_password: &str,
        crypto: &CryptoService,
    ) -> AppResult<()> {
        tracing::info!("[DB] update_account_password: id={}", account_id);

        if new_password.is_empty() {
            tracing::warn!(
                "[DB] update_account_password: empty password for id={}",
                account_id
            );
            return Err(AppError::Validation("密码不能为空".to_string()));
        }
        let encrypted = crypto.encrypt_string(new_password)?;
        let now = chrono::Utc::now().to_rfc3339();
        let model = accounts::ActiveModel {
            id: Set(account_id),
            password: Set(encrypted),
            updated_at: Set(now),
            ..Default::default()
        };
        model.update(&self.db).await?;
        Ok(())
    }

    /// 解密账号密码。
    pub fn decrypt_account_password(
        &self,
        account: &Account,
        crypto: &CryptoService,
    ) -> AppResult<String> {
        crypto.decrypt_string(&account.password)
    }

    /// 删除账号及其关联的原始账单和会话信息。
    pub async fn delete_account(&self, account_id: i64) -> AppResult<()> {
        tracing::warn!("[DB] delete_account: id={}", account_id);

        if let Some(acct) = self.get_account(account_id).await? {
            bill_original::Entity::delete_many()
                .filter(bill_original::Column::AccountId.eq(&acct.account_id))
                .exec(&self.db)
                .await?;
            session_info::Entity::delete_many()
                .filter(session_info::Column::AccountId.eq(&acct.account_id))
                .exec(&self.db)
                .await?;
        }
        accounts::Entity::delete_by_id(account_id)
            .exec(&self.db)
            .await?;

        tracing::info!("[DB] delete_account: id={} deleted", account_id);
        Ok(())
    }

    /// 更新账号的最后同步时间戳。
    pub async fn update_account_last_sync(&self, account_id: i64) -> AppResult<()> {
        tracing::debug!("[DB] update_account_last_sync: id={}", account_id);

        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let model = accounts::ActiveModel {
            id: Set(account_id),
            last_update_time: Set(now.clone()),
            updated_at: Set(now),
            ..Default::default()
        };
        model.update(&self.db).await?;
        Ok(())
    }

    // === 会话 CRUD ===

    /// 保存会话信息。先删除旧会话再插入新记录，确保每个账号只有一条有效会话。
    ///
    /// Cookies 经加密后存储，有效期默认 30 分钟。
    pub async fn save_session(
        &self,
        account_id: &str,
        cookies: &str,
        crypto: &CryptoService,
    ) -> AppResult<()> {
        tracing::info!("[DB] save_session: account_id={}", account_id);

        let encrypted_cookies = crypto.encrypt_string(cookies)?;
        let now = chrono::Local::now().to_rfc3339();
        let expire = (chrono::Local::now() + chrono::Duration::minutes(30)).to_rfc3339();

        session_info::Entity::delete_many()
            .filter(session_info::Column::AccountId.eq(account_id))
            .exec(&self.db)
            .await?;

        let model = session_info::ActiveModel {
            account_id: Set(account_id.to_string()),
            cookies: Set(encrypted_cookies),
            login_time: Set(Some(now)),
            expire_time: Set(Some(expire)),
            is_valid: Set(true),
            ..Default::default()
        };
        session_info::Entity::insert(model).exec(&self.db).await?;
        Ok(())
    }

    /// 获取指定账号的有效会话，解密 cookies 后返回。
    pub async fn get_session(
        &self,
        account_id: &str,
        crypto: &CryptoService,
    ) -> AppResult<Option<SessionInfo>> {
        tracing::debug!("[DB] get_session: account_id={}", account_id);

        let model = session_info::Entity::find()
            .filter(session_info::Column::AccountId.eq(account_id))
            .filter(session_info::Column::IsValid.eq(true))
            .one(&self.db)
            .await?;

        match model {
            Some(m) => {
                let decrypted = crypto.decrypt_string(&m.cookies)?;
                Ok(Some(SessionInfo {
                    id: m.id,
                    account_id: m.account_id,
                    cookies: decrypted,
                    login_time: m.login_time,
                    expire_time: m.expire_time,
                    is_valid: m.is_valid,
                }))
            }
            None => {
                tracing::debug!(
                    "[DB] get_session: no valid session for account_id={}",
                    account_id
                );
                Ok(None)
            }
        }
    }

    /// 将指定账号的会话标记为无效（不删除记录）。
    pub async fn invalidate_session(&self, account_id: &str) -> AppResult<()> {
        tracing::info!("[DB] invalidate_session: account_id={}", account_id);

        // Update using Column
        let _result = session_info::Entity::update_many()
            .col_expr(session_info::Column::IsValid, Expr::value(false))
            .filter(session_info::Column::AccountId.eq(account_id))
            .exec(&self.db)
            .await?;
        Ok(())
    }

    /// 删除指定账号的会话记录。
    pub async fn delete_session(&self, account_id: &str) -> AppResult<()> {
        tracing::info!("[DB] delete_session: account_id={}", account_id);

        session_info::Entity::delete_many()
            .filter(session_info::Column::AccountId.eq(account_id))
            .exec(&self.db)
            .await?;
        Ok(())
    }

    // === 数据清理操作 ===

    /// 清除指定账号的所有原始账单。
    pub async fn clear_account_original(&self, account_id: &str) -> AppResult<()> {
        tracing::info!("[DB] clear_account_original: account_id={}", account_id);

        bill_original::Entity::delete_many()
            .filter(bill_original::Column::AccountId.eq(account_id))
            .exec(&self.db)
            .await?;
        Ok(())
    }

    /// 清除指定身份+账号下的非手动合并账单。
    ///
    /// 仅删除 `is_manual=false` 的记录，保留用户手动添加的账单。
    pub async fn clear_merged_by_account(
        &self,
        identity_id: i64,
        account_id: &str,
    ) -> AppResult<()> {
        tracing::info!(
            "[DB] clear_merged_by_account: identity_id={}, account_id={}",
            identity_id,
            account_id
        );

        bill_merged::Entity::delete_many()
            .filter(bill_merged::Column::IdentityId.eq(identity_id))
            .filter(bill_merged::Column::SourceAccountId.eq(account_id))
            .filter(bill_merged::Column::IsManual.eq(false))
            .exec(&self.db)
            .await?;
        Ok(())
    }

    /// 清除指定身份下所有非手动合并账单。
    pub async fn clear_merged_non_manual(&self, identity_id: i64) -> AppResult<()> {
        tracing::info!("[DB] clear_merged_non_manual: identity_id={}", identity_id);

        bill_merged::Entity::delete_many()
            .filter(bill_merged::Column::IdentityId.eq(identity_id))
            .filter(bill_merged::Column::IsManual.eq(false))
            .exec(&self.db)
            .await?;
        Ok(())
    }

    /// 清除操作日志。若提供 account_id 则只删除该账号相关的日志。
    pub async fn clear_operation_logs(
        &self,
        identity_id: i64,
        account_id: Option<&str>,
    ) -> AppResult<()> {
        tracing::info!(
            "[DB] clear_operation_logs: identity_id={}, account_id={:?}",
            identity_id,
            account_id
        );

        let mut delete = operation_log::Entity::delete_many()
            .filter(operation_log::Column::IdentityId.eq(identity_id));
        if let Some(aid) = account_id {
            delete = delete.filter(operation_log::Column::AccountId.eq(aid));
        }
        delete.exec(&self.db).await?;
        Ok(())
    }
}

// === Model Conversion Helpers ===

/// 将数据库 identities 模型转换为应用层 Identity 模型。
fn identity_model_to_app(m: identities::Model) -> Identity {
    Identity {
        id: m.id,
        name: m.name,
        enable: m.enable,
        enable_update: m.enable_update,
        birthday: m.birthday,
        default_remember: m.default_remember,
        created_at: m.created_at,
        updated_at: m.updated_at,
    }
}

/// 将数据库 accounts 模型转换为应用层 Account 模型。
fn account_model_to_app(m: accounts::Model) -> Account {
    Account {
        id: m.id,
        identity_id: m.identity_id,
        account_name: m.account_name,
        account_id: m.account_id,
        password: m.password,
        enable: m.enable,
        enable_update: m.enable_update,
        admission_date: m.admission_date,
        graduation_date: m.graduation_date,
        expire_date: m.expire_date,
        last_update_time: m.last_update_time,
        created_at: m.created_at,
        updated_at: m.updated_at,
    }
}

/// 将数据库 bill_merged 模型转换为应用层 BillMerged 模型。
pub fn bill_merged_model_to_app(m: bill_merged::Model) -> BillMerged {
    BillMerged {
        id: m.id,
        date_str: m.date_str,
        time_str: m.time_str,
        time_str_formatted: m.time_str_formatted,
        date_time_formatted: m.date_time_formatted,
        end_date_time_formatted: m.end_date_time_formatted,
        timestamp: m.timestamp,
        end_timestamp: m.end_timestamp,
        item_type: m.item_type,
        number: m.number,
        number_list: m.number_list,
        target_user: m.target_user,
        money_str: m.money_str,
        money: m.money,
        method: m.method,
        status_str: m.status_str,
        is_combined: m.is_combined,
        source_account_id: m.source_account_id,
        is_manual: m.is_manual,
        position: m.position,
        room: m.room,
        category: m.category,
        notes: m.notes,
        synced_at: m.synced_at,
    }
}

/// 将数据库 bill_original 模型转换为应用层 BillOriginal 模型。
pub fn bill_original_model_to_app(m: bill_original::Model) -> BillOriginal {
    BillOriginal {
        id: m.id,
        date_str: m.date_str,
        time_str: m.time_str,
        time_str_formatted: m.time_str_formatted,
        date_time_formatted: m.date_time_formatted,
        end_date_time_formatted: m.end_date_time_formatted,
        timestamp: m.timestamp,
        end_timestamp: m.end_timestamp,
        item_type: m.item_type,
        number: m.number,
        number_list: m.number_list,
        target_user: m.target_user,
        money_str: m.money_str,
        money: m.money,
        method: m.method,
        status_str: m.status_str,
        is_combined: m.is_combined,
        account_id: m.account_id,
        synced_at: m.synced_at,
    }
}

/// 将数据库 operation_log 模型转换为应用层 OperationLog 模型。
pub fn operation_log_model_to_app(m: operation_log::Model) -> OperationLog {
    OperationLog {
        id: m.id,
        operation_type: m.operation_type,
        record_numbers: m.record_numbers,
        operation_time: m.operation_time,
        description: m.description,
        account_id: m.account_id,
    }
}
