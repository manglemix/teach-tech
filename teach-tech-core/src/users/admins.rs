use anyhow::Context;
use axum::http::StatusCode;
use axum::{response::IntoResponse, routing::get, Json};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use notifications::Notification;
use rand::distributions::{Alphanumeric, DistString};
use rand::rngs::OsRng;
use sea_orm::{entity::prelude::*, ActiveValue, TransactionTrait};
use serde::Serialize;
use tracing::error;
use zeroize::Zeroizing;

use crate::auth::user_auth::{self, new_from_password};
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

pub async fn create_admin(
    username: String,
    user_id: UserID,
    permissions: Vec<permissions::Permission>,
) -> anyhow::Result<()> {
    get_db()
        .transaction::<_, _, DbErr>(|txn| {
            Box::pin(async move {
                if let Some(_) = user_auth::Entity::find_by_id(user_id).one(get_db()).await? {
                    users::admins::ActiveModel {
                        user_id: ActiveValue::unchanged(user_id),
                        username: ActiveValue::set(username.clone()),
                        created_at: ActiveValue::not_set(),
                    }
                    .update(txn).await?;

                    println!(
                        "Created admin with user_id: {user_id}, username: {username}",
                    );
                } else {
                    let mut password = Zeroizing::new(String::new());
                    loop {
                        password.clear();
                        Alphanumeric.append_string(&mut OsRng, &mut password, 18);
                        match new_from_password(user_id, &password)
                            .await
                            .expect("Hashing admin password")
                            .insert(get_db())
                            .await
                        {
                            Ok(_) => break,
                            Err(DbErr::RecordNotInserted) => continue,
                            Err(e) => return Err(e),
                        }
                    }
                    users::admins::ActiveModel {
                        user_id: ActiveValue::set(user_id),
                        username: ActiveValue::set(username.clone()),
                        created_at: ActiveValue::set(chrono::Utc::now().naive_utc()),
                    }
                    .insert(txn).await?;

                    println!(
                        "Created admin with new user_id: {user_id}, username: {username}, password: {}",
                        &*password
                    );
                }

                permissions::Entity::delete_many().filter(permissions::Column::UserId.eq(user_id)).exec(txn).await?;

                for permission in permissions {
                    permissions::ActiveModel {
                        id: ActiveValue::not_set(),
                        user_id: ActiveValue::set(user_id),
                        permission: ActiveValue::set(permission),
                    }
                    .insert(txn)
                    .await?;
                }

                Ok(())
            })
        })
        .await
        .context("Creating admin")
}

#[derive(Debug, Serialize)]
pub struct AdminHome {
    #[serde(flatten)]
    pub model: Model,
    pub notifications: Vec<Notification>,
}

pub fn add_to_core<S: Clone + Send + Sync + 'static>(mut core: TeachCore<S>) -> TeachCore<S> {
    core.add_db_reset_config(Entity);
    core.add_db_reset_config(notifications::Entity);
    core.add_db_reset_config(permissions::Entity);

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
                    return (StatusCode::FORBIDDEN, ()).into_response();
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

pub mod permissions {
    use sea_orm::entity::prelude::*;

    use crate::auth::UserID;

    #[derive(Clone, Debug, DeriveEntityModel)]
    #[sea_orm(table_name = "admin_permissions")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub user_id: UserID,
        pub permission: Permission,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}

    #[derive(EnumIter, DeriveActiveEnum, Clone, Debug, Copy, PartialEq, Eq, clap::ValueEnum)]
    #[sea_orm(rs_type = "i32", db_type = "Integer")]
    pub enum Permission {
        CreateStudent = 0,
        DeleteStudent = 1,
        CreateInstructor = 2,
        DeleteInstructor = 3,
        CreateCourse = 4,
        DeleteCourse = 5,
        AssignInstructor = 6,
        CreateAdmin = 7,
        DeleteAdmin = 8,
    }
}
