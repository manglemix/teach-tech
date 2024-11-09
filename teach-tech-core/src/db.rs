use std::sync::OnceLock;

use anyhow::Context;
use sea_orm::{sea_query::Table, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, Schema};
use sea_orm_migration::SchemaManager;
use serde::Deserialize;

use crate::{auth, users};

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

pub async fn reset_db(config: &str) -> anyhow::Result<()> {
    let db_config: DBConfig = toml::from_str(config)?;
    let mut opt = ConnectOptions::new(db_config.database_url);
    opt.sqlx_logging(false);
    let conn = Database::connect(opt)
        .await
        .context("Connecting to database")?;
    let manager = SchemaManager::new(&conn);

    let mut drop = Table::drop();
    drop.table(users::admins::Entity).if_exists();
    manager.drop_table(drop).await?;

    drop = Table::drop();
    drop.table(users::students::Entity).if_exists();
    manager.drop_table(drop).await?;

    drop = Table::drop();
    drop.table(auth::token::Entity).if_exists();
    manager.drop_table(drop).await?;

    drop = Table::drop();
    drop.table(auth::user_auth::Entity).if_exists();
    manager.drop_table(drop).await?;

    drop = Table::drop();
    drop.table(users::admins::notifications::Entity).if_exists();
    manager.drop_table(drop).await?;

    let builder = conn.get_database_backend();
    let schema = Schema::new(builder);
    conn.execute(builder.build(&schema.create_table_from_entity(users::admins::Entity))).await?;
    conn.execute(builder.build(&schema.create_table_from_entity(users::students::Entity))).await?;
    conn.execute(builder.build(&schema.create_table_from_entity(auth::token::Entity))).await?;
    conn.execute(builder.build(&schema.create_table_from_entity(auth::user_auth::Entity))).await?;
    conn.execute(builder.build(&schema.create_table_from_entity(users::admins::notifications::Entity))).await?;
    Ok(())
}
