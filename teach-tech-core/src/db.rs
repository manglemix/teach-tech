use std::sync::OnceLock;

use anyhow::Context;
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use serde::Deserialize;

static MAIN_DB: OnceLock<DatabaseConnection> = OnceLock::new();

pub fn get_db() -> &'static DatabaseConnection {
    MAIN_DB
        .get()
        .expect("Database was not initialized. Call init_db first")
}

#[derive(Debug, Clone, Deserialize)]
pub struct DBConfig {
    pub database_url: String,
}

pub async fn init_db(config: &str) -> anyhow::Result<()> {
    let db_config: DBConfig = toml::from_str(config)?;
    let mut opt = ConnectOptions::new(db_config.database_url);
    opt.sqlx_logging(false);
    let conn = Database::connect(opt)
        .await
        .context("Connecting to database")?;
    MAIN_DB.set(conn).expect("Database is already initialized");
    Ok(())
}
