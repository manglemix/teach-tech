use fxhash::FxHashMap;
use sea_orm::prelude::*;
use serde::Serialize;
use teach_tech_core::{
    anyhow,
    auth::UserID,
    axum::{extract::WebSocketUpgrade, routing::get},
    TeachCore,
};

pub async fn add_to_core<S: Clone + Send + Sync + 'static>(
    mut core: TeachCore<S>,
) -> anyhow::Result<TeachCore<S>> {
    let mut info = FxHashMap::default();
    info.insert("version", env!("CARGO_PKG_VERSION"));
    core.add_info("quick-chat", info);
    core.add_db_reset_config(Entity);

    core = core.modify_router(|router| {
        router.route(
            "/quick-chat",
            get(|ws: WebSocketUpgrade| async { ws.on_upgrade(|ws| async move {}) }),
        )
    });

    core.add_on_serve(|| async move { Ok(()) });

    Ok(core)
}

#[derive(Clone, Debug, DeriveEntityModel, Serialize)]
#[sea_orm(table_name = "quick_chat_messages")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub user_id: i32,
    pub from: UserID,
    pub to: UserID,
    pub date: DateTime,
    pub message: String,
    pub read: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
