use anyhow::Context;
use rand::{distributions::{DistString, Standard}, rngs::OsRng};
use sea_orm::{entity::prelude::*, ActiveValue, TransactionTrait};

use crate::{auth::{token, user_auth, UserID}, db::get_db, users};


#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "admins")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub user_id: UserID,
    #[sea_orm(unique)]
    pub username: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

pub async fn create_admin(username: String) -> anyhow::Result<()> {
    get_db().transaction::<_, (), DbErr>(|txn| {
        Box::pin(async move {
            let mut user_id;
            let mut password = String::new();
            loop {
                user_id = UserID::rand();
                password.clear();
                Standard.append_string(&mut OsRng, &mut password, 18);
                match user_auth::new_from_password(user_id, &password).await.expect("Hashing admin password").insert(txn).await {
                    Ok(_) => break,
                    Err(DbErr::RecordNotInserted) => continue,
                    Err(e) => return Err(e)
                }
            }
            token::Model::gen_new(user_id, txn).await?.insert(txn).await?;

            users::admins::ActiveModel {
                user_id: ActiveValue::set(user_id),
                username: ActiveValue::set(username.clone())
            }.insert(txn).await?;

            println!("Created admin with user_id: {user_id}, username: {username}, password: {password}");
            Ok(())
        })
    }).await.context("Creating admin")
}