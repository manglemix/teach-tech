use argon2::{
    password_hash::{self, rand_core::OsRng, PasswordHasher, SaltString},
    Argon2, PasswordHash, PasswordVerifier,
};
use rand::distributions::{Alphanumeric, DistString};
use sea_orm::{entity::prelude::*, ActiveValue};
use zeroize::Zeroizing;

use super::UserID;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "user_auth")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub user_id: UserID,
    pub password_hash: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub fn validate_password(&self, password: &str) -> anyhow::Result<bool> {
        let parsed_hash = PasswordHash::new(&self.password_hash)
            .map_err(|e| anyhow::anyhow!("Parsing password hash for {}: {e:#}", self.user_id))?;
        match Argon2::default().verify_password(password.as_bytes(), &parsed_hash) {
            Ok(()) => Ok(true),
            Err(password_hash::Error::Password) => Ok(false),
            Err(e) => Err(anyhow::anyhow!(
                "Validating password for {}: {e:#}",
                self.user_id
            )),
        }
    }
}

pub async fn new_rand(conn: &impl ConnectionTrait) -> Result<(Model, Zeroizing<String>), DbErr> {
    let mut user_id;
    let mut password = Zeroizing::new(String::new());
    loop {
        user_id = UserID::rand();
        password.clear();
        Alphanumeric.append_string(&mut OsRng, &mut password, 18);
        match new_from_password(user_id, &password)
            .await
            .expect("Hashing admin password")
            .insert(conn)
            .await
        {
            Ok(m) => break Ok((m, password)),
            Err(DbErr::RecordNotInserted) => continue,
            Err(e) => return Err(e),
        }
    }
}

pub async fn new_from_password(
    user_id: UserID,
    password: &str,
) -> password_hash::Result<ActiveModel> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2.hash_password(password.as_bytes(), &salt)?;
    let password_hash = hash.to_string();

    Ok(ActiveModel {
        user_id: ActiveValue::set(user_id.into()),
        password_hash: ActiveValue::set(password_hash.clone()),
    })
}
