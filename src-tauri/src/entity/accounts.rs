use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "accounts")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub identity_id: i64,
    pub account_name: String,
    pub account_id: String,
    pub password: String,
    pub enable: bool,
    pub enable_update: bool,
    pub expire_date: String,
    pub last_update_time: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::identities::Entity",
        from = "Column::IdentityId",
        to = "super::identities::Column::Id"
    )]
    Identity,
}

impl Related<super::identities::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Identity.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
