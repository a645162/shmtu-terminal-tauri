use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "identities")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub enable: bool,
    pub enable_update: bool,
    pub birthday: Option<String>,
    pub default_remember: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::accounts::Entity")]
    Accounts,
    #[sea_orm(has_many = "super::bill_merged::Entity")]
    BillMerged,
    #[sea_orm(has_many = "super::operation_log::Entity")]
    OperationLog,
}

impl Related<super::accounts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Accounts.def()
    }
}

impl Related<super::bill_merged::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::BillMerged.def()
    }
}

impl Related<super::operation_log::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::OperationLog.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
