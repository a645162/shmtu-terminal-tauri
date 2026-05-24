use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "session_info")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub account_id: String,
    pub cookies: String,
    pub login_time: Option<String>,
    pub expire_time: Option<String>,
    pub is_valid: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::accounts::Entity",
        from = "Column::AccountId",
        to = "super::accounts::Column::AccountId"
    )]
    Account,
}

impl Related<super::accounts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Account.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
