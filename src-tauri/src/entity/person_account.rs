use sea_orm::entity::prelude::*;

/// 一卡通个人账户缓存（每个账号一条）
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "person_account_cache")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    /// 关联的账号 ID（学号）
    pub account_id: String,
    pub real_name: String,
    pub real_name_auth_status: String,
    pub cash_balance: f64,
    pub cash_balance_raw: String,
    pub security_question_status: String,
    pub register_date: String,
    pub student_id: String,
    pub email: String,
    pub nickname: String,
    pub gender: String,
    pub class_name: String,
    pub phone_num: String,
    pub id_type: String,
    pub id_number: String,
    pub remark: String,
    pub user_type: String,
    pub csrf_token: String,
    pub csrf_header: String,
    pub fetched_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
