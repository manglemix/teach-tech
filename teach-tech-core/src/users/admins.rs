use anyhow::Context;
use axum::http::StatusCode;
use axum::{response::IntoResponse, routing::get, Json};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use notifications::Notification;
use rand::{
    distributions::{Alphanumeric, DistString},
    rngs::OsRng,
};
use sea_orm::{entity::prelude::*, ActiveValue, TransactionTrait};
use serde::Serialize;
use tracing::error;

use crate::{
    auth::{token, user_auth, UserID},
    db::get_db,
    users, TeachCore,
};

#[derive(Clone, Debug, DeriveEntityModel)]
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
                Alphanumeric.append_string(&mut OsRng, &mut password, 18);
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

#[derive(Debug, Serialize)]
pub struct AdminHome {
    pub user_id: UserID,
    pub username: String,
    pub notifications: Vec<Notification>,
}

pub async fn add_to_core<S: Clone + Send + Sync + 'static>(core: TeachCore<S>) -> TeachCore<S> {
    core.modify_router(|router| {
        router.route("/admin/home", get(|TypedHeader(Authorization(bearer)): TypedHeader<Authorization<Bearer>>| async move {
            let token = match token::Entity::find_by_id(bearer.token()).one(get_db()).await {
                Ok(Some(t)) => t,
                Ok(None) => return (StatusCode::UNAUTHORIZED, ()).into_response(),
                Err(e) => {
                    error!("Error validating bearer token: {e:#}");
                    return (StatusCode::INTERNAL_SERVER_ERROR, ()).into_response();
                }
            };
            let username = match Entity::find_by_id(token.user_id).one(get_db()).await {
                Ok(Some(m)) => m.username,
                Ok(None) => {
                    error!("User id {} not found in admins table, but bearer token was valid", token.user_id);
                    return (StatusCode::UNAUTHORIZED, ()).into_response();
                }
                Err(e) => {
                    error!("Error reading admin data: {e:#}");
                    return (StatusCode::INTERNAL_SERVER_ERROR, ()).into_response();
                }
            };

            let user_id = token.user_id;
            if let Err(e) = token.update_last_used(get_db()).await {
                error!("Error updating token last used time: {e:#}");
            }

            let notifications: Vec<_> = match notifications::Entity::find_by_id(user_id).all(get_db()).await {
                Ok(n) => n.into_iter().map(Notification::from).collect(),
                Err(e) => {
                    error!("Error reading admin notifications: {e:#}");
                    return (StatusCode::INTERNAL_SERVER_ERROR, ()).into_response();
                }
            };

            (StatusCode::OK, Json(AdminHome { user_id, username, notifications })).into_response()
        }))
    })
}

pub mod notifications {
    use serde::Serialize;

    use super::*;

    #[derive(Clone, Debug, Serialize)]
    pub struct Notification {
        pub severity: String,
        pub message: String,
    }

    impl From<Model> for Notification {
        fn from(m: Model) -> Self {
            Self {
                severity: m.severity,
                message: m.message,
            }
        }
    }

    #[derive(Clone, Debug, DeriveEntityModel)]
    #[sea_orm(table_name = "admin_notifications")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub user_id: UserID,
        pub severity: String,
        pub message: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}
