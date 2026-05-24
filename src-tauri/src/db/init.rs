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

#[derive(Clone)]
pub struct DatabaseManager {
    db: DatabaseConnection,
    data_dir: PathBuf,
}

impl DatabaseManager {
    pub fn clone_ref(&self) -> Self {
        self.clone()
    }

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
        let db = sea_orm::Database::connect(&db_url)
            .await
            .map_err(|e| AppError::Database(format!("数据库连接失败: {}", e)))?;

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

    async fn init_tables(&self) -> AppResult<()> {
        use sea_orm::Statement;

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
        self.account_dir().join(format!("{}_session.sqlite", account_id))
    }
    // === 身份 CRUD ===

    pub async fn list_identities(&self) -> AppResult<Vec<Identity>> {
        let models = identities::Entity::find()
            .order_by_asc(identities::Column::Id)
            .all(&self.db)
            .await?;

        Ok(models.into_iter().map(identity_model_to_app).collect())
    }

    pub async fn create_identity(&self, params: &CreateIdentityParams) -> AppResult<i64> {
        let count = identities::Entity::find()
            .filter(identities::Column::Name.eq(&params.name))
            .count(&self.db)
            .await?;
        if count > 0 {
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
        Ok(result.last_insert_id)
    }

    pub async fn update_identity(&self, identity: &Identity) -> AppResult<()> {
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

    pub async fn delete_identity(&self, identity_id: i64) -> AppResult<()> {
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
        Ok(())
    }

    pub async fn get_identity(&self, identity_id: i64) -> AppResult<Option<Identity>> {
        let model = identities::Entity::find_by_id(identity_id)
            .one(&self.db)
            .await?;
        Ok(model.map(identity_model_to_app))
    }

    // === 账号 CRUD ===

    pub async fn list_all_accounts(&self) -> AppResult<Vec<Account>> {
        let models = accounts::Entity::find()
            .order_by_asc(accounts::Column::Id)
            .all(&self.db)
            .await?;
        Ok(models.into_iter().map(account_model_to_app).collect())
    }

    pub async fn list_accounts_by_identity(&self, identity_id: i64) -> AppResult<Vec<Account>> {
        let models = accounts::Entity::find()
            .filter(accounts::Column::IdentityId.eq(identity_id))
            .order_by_asc(accounts::Column::Id)
            .all(&self.db)
            .await?;
        Ok(models.into_iter().map(account_model_to_app).collect())
    }

    pub async fn create_account(
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

        let model = accounts::ActiveModel {
            identity_id: Set(params.identity_id),
            account_name: Set(params.account_name.clone()),
            account_id: Set(params.account_id.clone()),
            password: Set(encrypted_password),
            enable: Set(params.enable.unwrap_or(true)),
            enable_update: Set(params.enable_update.unwrap_or(true)),
            expire_date: Set(params.expire_date.as_deref().unwrap_or("2099-12-31").to_string()),
            last_update_time: Set(String::new()),
            created_at: Set(now.clone()),
            updated_at: Set(now),
            ..Default::default()
        };

        let result = accounts::Entity::insert(model).exec(&self.db).await?;
        Ok(result.last_insert_id)
    }

    pub async fn get_account(&self, account_id: i64) -> AppResult<Option<Account>> {
        let model = accounts::Entity::find_by_id(account_id)
            .one(&self.db)
            .await?;
        Ok(model.map(account_model_to_app))
    }

    pub async fn get_account_by_student_id(&self, student_id: &str) -> AppResult<Option<Account>> {
        let model = accounts::Entity::find()
            .filter(accounts::Column::AccountId.eq(student_id))
            .one(&self.db)
            .await?;
        Ok(model.map(account_model_to_app))
    }

    pub async fn update_account(&self, account: &Account, crypto: &CryptoService) -> AppResult<()> {
        let now = chrono::Utc::now().to_rfc3339();

        if account.password.is_empty() {
            let model = accounts::ActiveModel {
                id: Set(account.id),
                account_name: Set(account.account_name.clone()),
                enable: Set(account.enable),
                enable_update: Set(account.enable_update),
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
                expire_date: Set(account.expire_date.clone()),
                last_update_time: Set(account.last_update_time.clone()),
                updated_at: Set(now),
                ..Default::default()
            };
            model.update(&self.db).await?;
        }

        Ok(())
    }

    pub async fn update_account_password(
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
        let model = accounts::ActiveModel {
            id: Set(account_id),
            password: Set(encrypted),
            updated_at: Set(now),
            ..Default::default()
        };
        model.update(&self.db).await?;
        Ok(())
    }

    pub fn decrypt_account_password(
        &self,
        account: &Account,
        crypto: &CryptoService,
    ) -> AppResult<String> {
        crypto.decrypt_string(&account.password)
    }

    pub async fn delete_account(&self, account_id: i64) -> AppResult<()> {
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
        Ok(())
    }

    pub async fn update_account_last_sync(&self, account_id: i64) -> AppResult<()> {
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

    pub async fn save_session(
        &self,
        account_id: &str,
        cookies: &str,
        crypto: &CryptoService,
    ) -> AppResult<()> {
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

    pub async fn get_session(
        &self,
        account_id: &str,
        crypto: &CryptoService,
    ) -> AppResult<Option<SessionInfo>> {
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
            None => Ok(None),
        }
    }

    pub async fn invalidate_session(&self, account_id: &str) -> AppResult<()> {
        // Update using Column
        let _result = session_info::Entity::update_many()
            .col_expr(session_info::Column::IsValid, Expr::value(false))
            .filter(session_info::Column::AccountId.eq(account_id))
            .exec(&self.db)
            .await?;
        Ok(())
    }

    pub async fn delete_session(&self, account_id: &str) -> AppResult<()> {
        session_info::Entity::delete_many()
            .filter(session_info::Column::AccountId.eq(account_id))
            .exec(&self.db)
            .await?;
        Ok(())
    }

    // === 数据清理操作 ===

    pub async fn clear_account_original(&self, account_id: &str) -> AppResult<()> {
        bill_original::Entity::delete_many()
            .filter(bill_original::Column::AccountId.eq(account_id))
            .exec(&self.db)
            .await?;
        Ok(())
    }

    pub async fn clear_merged_by_account(&self, identity_id: i64, account_id: &str) -> AppResult<()> {
        bill_merged::Entity::delete_many()
            .filter(bill_merged::Column::IdentityId.eq(identity_id))
            .filter(bill_merged::Column::SourceAccountId.eq(account_id))
            .filter(bill_merged::Column::IsManual.eq(false))
            .exec(&self.db)
            .await?;
        Ok(())
    }

    pub async fn clear_merged_non_manual(&self, identity_id: i64) -> AppResult<()> {
        bill_merged::Entity::delete_many()
            .filter(bill_merged::Column::IdentityId.eq(identity_id))
            .filter(bill_merged::Column::IsManual.eq(false))
            .exec(&self.db)
            .await?;
        Ok(())
    }

    pub async fn clear_operation_logs(
        &self,
        identity_id: i64,
        account_id: Option<&str>,
    ) -> AppResult<()> {
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

fn account_model_to_app(m: accounts::Model) -> Account {
    Account {
        id: m.id,
        identity_id: m.identity_id,
        account_name: m.account_name,
        account_id: m.account_id,
        password: m.password,
        enable: m.enable,
        enable_update: m.enable_update,
        expire_date: m.expire_date,
        last_update_time: m.last_update_time,
        created_at: m.created_at,
        updated_at: m.updated_at,
    }
}

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
        notes: m.notes,
        synced_at: m.synced_at,
    }
}

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
