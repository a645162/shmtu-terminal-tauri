use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "operation_log")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub identity_id: i64,
    pub operation_type: String,
    pub record_numbers: Option<String>,
    pub operation_time: String,
    pub description: Option<String>,
    pub account_id: Option<String>,
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
