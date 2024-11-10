use axum::{
    extract::Json,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use sea_orm::{entity::prelude::*, ActiveValue, TransactionTrait};
use serde::{Deserialize, Serialize};
use tracing::error;
use zeroize::Zeroizing;

use crate::{
    auth::{token, user_auth, UserID},
    db::get_db,
    TeachCore,
};

use super::admins;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize)]
#[sea_orm(table_name = "instructors")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub user_id: UserID,
    pub name: String,
    pub pronouns: String,
    pub birthdate: DateTime,
    pub created_at: DateTime,
    #[serde(skip_serializing)]
    pub created_by: UserID,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Deserialize)]
pub struct CreateInstructor {
    pub name: String,
    pub birthdate: chrono::DateTime<chrono::Utc>,
    pub pronouns: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateInstructors {
    pub instructors: Vec<CreateInstructor>,
}

#[derive(Debug, Serialize)]
pub struct CreatedInstructor {
    pub user_id: UserID,
    pub password: Zeroizing<String>,
}

#[derive(Debug, Serialize)]
pub struct CreatedInstructors {
    pub instructors: Vec<CreatedInstructor>,
}

#[derive(Debug, Serialize)]
pub struct InstructorHome {
    #[serde(flatten)]
    pub model: Model,
}

pub fn add_to_core<S: Clone + Send + Sync + 'static>(mut core: TeachCore<S>) -> TeachCore<S> {
    core.add_db_reset_config(Entity);
    core.add_db_reset_config(permissions::Entity);

    core.modify_router(|router| {
        router.route("/instructor/home", get(|TypedHeader(Authorization(bearer)): TypedHeader<Authorization<Bearer>>| async move {
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
                    error!("Error reading instructor data: {e:#}");
                    return (StatusCode::INTERNAL_SERVER_ERROR, ()).into_response();
                }
            };

            let user_id = token.user_id;
            if let Err(e) = token.update_last_used(get_db()).await {
                error!("Error updating token last used time for {user_id}: {e:#}");
            }

            (StatusCode::OK, Json(InstructorHome { model })).into_response()
        }))
        .route("/instructor/create", post(|TypedHeader(Authorization(bearer)): TypedHeader<Authorization<Bearer>>, Json(CreateInstructors { instructors }): Json<CreateInstructors>| async move {
            let token = match token::Entity::find_by_id(bearer.token()).one(get_db()).await {
                Ok(Some(t)) => t,
                Ok(None) => return (StatusCode::UNAUTHORIZED, ()).into_response(),
                Err(e) => {
                    error!("Error validating bearer token: {e:#}");
                    return (StatusCode::INTERNAL_SERVER_ERROR, ()).into_response();
                }
            };

            match admins::permissions::Entity::find().filter(admins::permissions::Column::UserId.eq(token.user_id)).filter(admins::permissions::Column::Permission.eq(admins::permissions::Permission::CreateInstructor)).one(get_db()).await {
                Ok(Some(_)) => {}
                Ok(None) => {
                    return (StatusCode::FORBIDDEN, "Must be an administrator that can create instructors").into_response();
                }
                Err(e) => {
                    error!("Error reading admin data: {e:#}");
                    return (StatusCode::INTERNAL_SERVER_ERROR, ()).into_response();
                }
            }

            let user_id = token.user_id;
            if let Err(e) = token.update_last_used(get_db()).await {
                error!("Error updating token last used time for {user_id}: {e:#}");
            }

            let result = get_db().transaction::<_, _, DbErr>(|txn| {
                Box::pin(async move {
                    let mut created_instructors = vec![];
                    let created_at = chrono::Utc::now().naive_utc();
                    for instructor in instructors {
                        let (instructor_auth, password) = user_auth::new_rand(txn).await?;

                        ActiveModel {
                            user_id: ActiveValue::Set(instructor_auth.user_id),
                            name: ActiveValue::Set(instructor.name),
                            pronouns: ActiveValue::Set(instructor.pronouns),
                            birthdate: ActiveValue::Set(instructor.birthdate.naive_utc()),
                            created_at: ActiveValue::Set(created_at),
                            created_by: ActiveValue::Set(user_id),
                        }.insert(txn).await?;

                        created_instructors.push(CreatedInstructor { user_id: instructor_auth.user_id, password });
                    }
                    Ok(created_instructors)
                })
            }).await;

            match result {
                Ok(instructors) => {
                    (StatusCode::OK, Json(CreatedInstructors { instructors })).into_response()
                }
                Err(e) => {
                    error!("Error creating instructors: {e:#}");
                    (StatusCode::INTERNAL_SERVER_ERROR, ()).into_response()
                }
            }
        }))
    })
}

pub mod permissions {
    use sea_orm::entity::prelude::*;

    use crate::auth::UserID;

    #[derive(Clone, Debug, DeriveEntityModel)]
    #[sea_orm(table_name = "instructor_permissions")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub user_id: UserID,
        pub permission: Permission,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}

    #[derive(EnumIter, DeriveActiveEnum, Clone, Debug, Copy, PartialEq, Eq)]
    #[sea_orm(rs_type = "i32", db_type = "Integer")]
    pub enum Permission {
        ViewGrades = 0,
        SetGrades = 1,
        GradeAssignment = 2,
        CreateAssignment = 3,
        ModifyRubric = 4,
    }
}
