use anyhow::Context;
use axum::http::StatusCode;
use axum::{response::IntoResponse, routing::get, Json};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use notifications::Notification;
use sea_orm::{entity::prelude::*, ActiveValue, TransactionTrait};
use serde::Serialize;
use tracing::error;

use crate::auth::user_auth::new_rand;
use crate::{
    auth::{token, UserID},
    db::get_db,
    users, TeachCore,
};

#[derive(Clone, Debug, DeriveEntityModel, Serialize)]
#[sea_orm(table_name = "admins")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub user_id: UserID,
    #[sea_orm(unique)]
    pub username: String,
    pub created_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

pub async fn create_admin(username: String) -> anyhow::Result<()> {
    get_db().transaction::<_, _, DbErr>(|txn| {
        Box::pin(async move {
            let (model, password) = new_rand(txn).await?;
            let user_id = model.user_id;

            users::admins::ActiveModel {
                user_id: ActiveValue::set(user_id),
                username: ActiveValue::set(username.clone()),
                created_at: ActiveValue::set(chrono::Utc::now().naive_utc()),
            }.insert(txn).await?;

            println!("Created admin with user_id: {user_id}, username: {username}, password: {}", &*password);
            Ok(())
        })
    }).await.context("Creating admin")
}

#[derive(Debug, Serialize)]
pub struct AdminHome {
    #[serde(flatten)]
    pub model: Model,
    pub notifications: Vec<Notification>,
}

pub fn add_to_core<S: Clone + Send + Sync + 'static>(core: TeachCore<S>) -> TeachCore<S> {
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
            let model = match Entity::find_by_id(token.user_id).one(get_db()).await {
                Ok(Some(m)) => m,
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
                error!("Error updating token last used time for {user_id}: {e:#}");
            }

            let notifications: Vec<_> = match notifications::Entity::find_by_id(user_id).all(get_db()).await {
                Ok(n) => n.into_iter().map(Notification::from).collect(),
                Err(e) => {
                    error!("Error reading admin notifications: {e:#}");
                    return (StatusCode::INTERNAL_SERVER_ERROR, ()).into_response();
                }
            };

            (StatusCode::OK, Json(AdminHome { model, notifications })).into_response()
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
