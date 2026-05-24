use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "bill_merged")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub identity_id: i64,
    pub date_str: String,
    pub time_str: String,
    pub time_str_formatted: Option<String>,
    pub date_time_formatted: Option<String>,
    pub end_date_time_formatted: Option<String>,
    pub timestamp: Option<i64>,
    pub end_timestamp: Option<i64>,
    pub item_type: Option<String>,
    pub number: Option<String>,
    pub number_list: Option<String>,
    pub target_user: Option<String>,
    pub money_str: Option<String>,
    pub money: Option<f64>,
    pub method: Option<String>,
    pub status_str: Option<String>,
    pub is_combined: bool,
    pub source_account_id: Option<String>,
    pub is_manual: bool,
    pub synced_at: Option<String>,
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
