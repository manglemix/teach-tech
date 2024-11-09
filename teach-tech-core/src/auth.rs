pub mod user_auth;
pub mod token;

use axum::{http::StatusCode, response::IntoResponse, routing::post, Form, Json};
use rand::{thread_rng, Rng};
use sea_orm::{entity::prelude::*, TryFromU64};
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::{db::get_db, TeachCore};

#[derive(Clone, Copy, Debug, PartialEq, Eq, DeriveValueType, Serialize, Deserialize)]
pub struct UserID(i32);


impl TryFromU64 for UserID {
    fn try_from_u64(n: u64) -> Result<Self, DbErr> {
        Ok(Self(i32::try_from_u64(n)?))
    }
}

impl UserID {
    pub fn rand() -> Self {
        let n: i32 = thread_rng().gen();
        Self(n.abs())
    }
}


impl From<UserID> for u32 {
    fn from(value: UserID) -> Self {
        value.0 as u32
    }
}


impl From<UserID> for i32 {
    fn from(value: UserID) -> Self {
        value.0
    }
}


impl TryFrom<u32> for UserID {
    type Error = <i32 as TryFrom<u32>>::Error;

    fn try_from(n: u32) -> Result<Self, Self::Error> {
        Ok(Self(i32::try_from(n)?))
    }
}


impl TryFrom<i32> for UserID {
    type Error = <u32 as TryFrom<i32>>::Error;

    fn try_from(n: i32) -> Result<Self, Self::Error> {
        Ok(Self(u32::try_from(n)? as i32))
    }
}


impl std::fmt::Display for UserID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoginForm {
    pub user_id: UserID,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct Token {
    pub token: String,
    pub expires_at: DateTime,
}

pub async fn add_to_core<S: Clone + Send + Sync + 'static>(core: TeachCore<S>) -> anyhow::Result<TeachCore<S>> {
    Ok(core.modify_router(|router| {
        router.route("/auth/login", post(|Form(LoginForm { user_id, password }): Form<LoginForm>| async move {
            let auth_data = match user_auth::Entity::find_by_id(user_id).one(get_db()).await {
                Ok(Some(auth_data)) => auth_data,
                Ok(None) => return (StatusCode::UNAUTHORIZED, ()).into_response(),
                Err(e) => {
                    error!("Error getting user auth data for {user_id}: {e:#}");
                    return (StatusCode::INTERNAL_SERVER_ERROR, ()).into_response();
                }
            };
            match auth_data.validate_password(&password) {
                Ok(true) => { }
                Ok(false) => return (StatusCode::UNAUTHORIZED, ()).into_response(),
                Err(e) => {
                    error!("Error validating user: {e:#}");
                    return (StatusCode::INTERNAL_SERVER_ERROR, ()).into_response();
                }
            }

            let result = match token::Model::gen_new(user_id, get_db()).await {
                Ok(m) => Ok(m.insert(get_db()).await),
                Err(e) => Err(e)
            };

            match result {
                Ok(Ok(token)) => {
                    let expiry = chrono::Utc::now().naive_utc() + token::get_token_validity_duration_std();
                    (StatusCode::OK, Json(Token {
                        token: token.token,
                        expires_at: expiry
                    })).into_response()
                },
                Ok(Err(e)) | Err(e) => {
                    error!("Error creating token for {user_id}: {e:#}");
                    (StatusCode::INTERNAL_SERVER_ERROR, ()).into_response()
                }
            }
        }))
    }))
}