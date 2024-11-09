use axum::{extract::Json, http::StatusCode, response::IntoResponse, routing::{get, post}};
use axum_extra::{headers::{authorization::Bearer, Authorization}, TypedHeader};
use sea_orm::{entity::prelude::*, ActiveValue, TransactionTrait};
use serde::{Deserialize, Serialize};
use tracing::error;
use zeroize::Zeroizing;

use crate::{auth::{token, user_auth, UserID}, db::get_db, TeachCore};

use super::admins;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize)]
#[sea_orm(table_name = "students")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub user_id: UserID,
    pub name: String,
    pub pronouns: String,
    pub birthday: DateTime,
    pub created_at: DateTime,
    #[serde(skip_serializing)]
    pub created_by: UserID,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Deserialize)]
pub struct CreateStudent {
    pub name: String,
    pub birthday: chrono::DateTime<chrono::Utc>,
    pub pronouns: String
}

#[derive(Debug, Deserialize)]
pub struct CreateStudents {
    pub students: Vec<CreateStudent>
}

#[derive(Debug, Serialize)]
pub struct CreatedStudent {
    pub user_id: UserID,
    pub password: Zeroizing<String>
}

#[derive(Debug, Serialize)]
pub struct CreatedStudents {
    pub students: Vec<CreatedStudent>
}

#[derive(Debug, Serialize)]
pub struct StudentHome {
    #[serde(flatten)]
    pub model: Model
}

pub fn add_to_core<S: Clone + Send + Sync + 'static>(core: TeachCore<S>) -> TeachCore<S> {
    core.modify_router(|router| {
        router.route("/student/home", get(|TypedHeader(Authorization(bearer)): TypedHeader<Authorization<Bearer>>| async move {
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
                    error!("User id {} not found in students table, but bearer token was valid", token.user_id);
                    return (StatusCode::FORBIDDEN, ()).into_response();
                }
                Err(e) => {
                    error!("Error reading student data: {e:#}");
                    return (StatusCode::INTERNAL_SERVER_ERROR, ()).into_response();
                }
            };

            let user_id = token.user_id;
            if let Err(e) = token.update_last_used(get_db()).await {
                error!("Error updating token last used time for {user_id}: {e:#}");
            }

            (StatusCode::OK, Json(StudentHome { model })).into_response()
        }))
        .route("/student/create", post(|TypedHeader(Authorization(bearer)): TypedHeader<Authorization<Bearer>>, Json(CreateStudents { students }): Json<CreateStudents>| async move {
            let token = match token::Entity::find_by_id(bearer.token()).one(get_db()).await {
                Ok(Some(t)) => t,
                Ok(None) => return (StatusCode::UNAUTHORIZED, ()).into_response(),
                Err(e) => {
                    error!("Error validating bearer token: {e:#}");
                    return (StatusCode::INTERNAL_SERVER_ERROR, ()).into_response();
                }
            };

            match admins::Entity::find_by_id(token.user_id).one(get_db()).await {
                Ok(Some(_)) => {}
                Ok(None) => {
                    return (StatusCode::FORBIDDEN, "Must be an administrator").into_response();
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
                    let mut created_students = vec![];
                    let created_at = chrono::Utc::now().naive_utc();
                    for student in students {
                        let (student_auth, password) = user_auth::new_rand(txn).await?;

                        ActiveModel {
                            user_id: ActiveValue::Set(student_auth.user_id),
                            name: ActiveValue::Set(student.name),
                            pronouns: ActiveValue::Set(student.pronouns),
                            birthday: ActiveValue::Set(student.birthday.naive_utc()),
                            created_at: ActiveValue::Set(created_at),
                            created_by: ActiveValue::Set(user_id),
                        }.insert(txn).await?;

                        created_students.push(CreatedStudent { user_id: student_auth.user_id, password });
                    }
                    Ok(created_students)
                })
            }).await;

            match result {
                Ok(students) => {
                    (StatusCode::OK, Json(CreatedStudents { students })).into_response()
                }
                Err(e) => {
                    error!("Error creating students: {e:#}");
                    (StatusCode::INTERNAL_SERVER_ERROR, ()).into_response()
                }
            }
        }))
    })
}
