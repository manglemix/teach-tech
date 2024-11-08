use anyhow::Context;
use crossbeam::atomic::AtomicCell;
use rand::{distributions::{Alphanumeric, DistString}, rngs::OsRng};
use sea_orm::{entity::prelude::*, ActiveValue};

use crate::db::get_db;

use super::UserID;


static VALIDITY_DURATION: AtomicCell<chrono::Duration> = AtomicCell::new(chrono::Duration::days(3));

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "user_auth_tokens")]
pub struct Model {
    #[sea_orm(unique)]
    pub user_id: UserID,
    #[sea_orm(primary_key, auto_increment = false)]
    pub token: String,
    pub last_used: DateTime
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}


impl Model {
    pub async fn gen_new(user_id: UserID, db: &impl ConnectionTrait) -> Result<ActiveModel, DbErr> {
        if let Some(model) = Entity::find().filter(Column::UserId.eq(user_id)).one(db).await? {
            model.delete(db).await?;
        }
        
        let mut token = String::new();
        Alphanumeric.append_string(&mut OsRng, &mut token, 32);
        
        Ok(ActiveModel {
            user_id: ActiveValue::set(user_id),
            token: ActiveValue::set(token),
            last_used: ActiveValue::set(chrono::Utc::now().naive_utc())
        })
    }
}

pub async fn validate_token(token: &str) -> anyhow::Result<Option<UserID>> {
    let Some(model) = Entity::find_by_id(token).one(get_db()).await? else {
        return Ok(None);
    };
    
    let now = chrono::Utc::now().naive_utc();
    let elapsed = now - model.last_used;
    if elapsed > VALIDITY_DURATION.load() {
        let user_id = model.user_id;
        model.delete(get_db()).await.with_context(|| format!("Deleting expired token for {user_id}"))?;
        return Ok(None);
    }
    ActiveModel {
        user_id: ActiveValue::unchanged(model.user_id),
        token: ActiveValue::not_set(),
        last_used: ActiveValue::set(now)
    }.update(get_db()).await.with_context(|| format!("Updating token for {}", model.user_id))?;
    
    Ok(Some(model.user_id))
}