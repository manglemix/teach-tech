#![feature(duration_constructors)]
#![feature(impl_trait_in_assoc_type)]
#![feature(build_hasher_default_const_new)]
#![feature(const_collections_with_hasher)]
#![feature(try_blocks)]

use std::{
    any::Any,
    future::Future,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::Path,
    pin::Pin,
    process::ExitCode,
    sync::Arc,
};

use anyhow::Context;
use axum::{body::Body, response::Response, routing::get, Router};
use clap::{Parser, Subcommand};
use db::{get_db, init_db};
use fxhash::FxHashMap;
use sea_orm::{
    sea_query::{IntoTableRef, Table, TableCreateStatement, TableDropStatement},
    ConnectionTrait, EntityTrait, Schema,
};
use sea_orm_migration::SchemaManager;
use serde::{Deserialize, Serialize};
use serde_json::to_value;
use tokio::sync::Notify;
use tower_http::{compression, cors, decompression, trace};
use tracing::error;
use tracing_subscriber::EnvFilter;
use users::admins::create_admin;

pub use anyhow;
pub use axum;
pub use serde_json;
pub use tokio;

pub mod auth;
pub mod db;
pub mod siblings;
pub mod users;

#[derive(Debug, Clone, Deserialize)]
pub struct ApiConfig {
    #[serde(default = "default_server_address")]
    pub server_address: SocketAddr,
}

fn default_server_address() -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 80)
}

pub struct TeachCore<S = ()> {
    router: Router<S>,
    schema: Schema,
    reset_db: Vec<(TableDropStatement, TableCreateStatement)>,
    config: String,
    info: FxHashMap<String, serde_json::Value>,
    on_serve: Vec<Box<dyn FnOnce() -> Pin<Box<dyn Future<Output = anyhow::Result<()>>>> + Send>>,
    to_drop: Vec<Box<dyn Any>>,
}

impl<S> TeachCore<S> {
    pub fn get_config_str(&self) -> &str {
        &self.config
    }

    pub fn add_db_reset_config(&mut self, entity: impl IntoTableRef + EntityTrait) {
        let mut drop = Table::drop();
        drop.table(entity).if_exists();
        let create = self.schema.create_table_from_entity(entity);
        self.reset_db.push((drop, create));
    }

    pub fn add_info(&mut self, name: impl Into<String>, value: impl Serialize) {
        let name = name.into();
        let value = to_value(value).expect("Serializing info value");
        if self.info.insert(name.clone(), value).is_some() {
            panic!("Duplicate info key: {}", name);
        }
    }

    pub fn modify_router<T>(self, f: impl FnOnce(Router<S>) -> Router<T>) -> TeachCore<T> {
        TeachCore {
            router: f(self.router),
            info: self.info,
            schema: self.schema,
            reset_db: self.reset_db,
            config: self.config,
            on_serve: self.on_serve,
            to_drop: self.to_drop,
        }
    }

    pub fn add_on_serve<Fut>(&mut self, f: impl FnOnce() -> Fut + Send + 'static)
    where
        Fut: Future<Output = anyhow::Result<()>> + 'static,
    {
        self.on_serve.push(Box::new(|| Box::pin(f())));
    }

    pub fn add_to_drop(&mut self, x: impl Any) {
        self.to_drop.push(Box::new(x));
    }

    pub async fn reset_db(self) -> anyhow::Result<ExitCode> {
        let manager = SchemaManager::new(get_db());
        let builder = get_db().get_database_backend();

        for (drop, create) in self.reset_db {
            manager.drop_table(drop).await?;
            get_db().execute(builder.build(&create)).await?;
        }
        Ok(ExitCode::SUCCESS)
    }
}

impl TeachCore<()> {
    pub async fn serve(self) -> anyhow::Result<ExitCode> {
        let api_config: ApiConfig =
            toml::from_str(self.get_config_str()).context("Parsing teach-config.toml")?;

        let listener = tokio::net::TcpListener::bind(api_config.server_address)
            .await
            .with_context(|| format!("Binding to {}", api_config.server_address))?;

        let cors = cors::CorsLayer::new().allow_methods(cors::Any);

        #[cfg(debug_assertions)]
        let cors = cors.allow_origin(cors::Any).allow_headers(cors::Any);
        let router = self.router;
        #[cfg(debug_assertions)]
        let router = router.layer(hot_reload::HotReloadLayer::default());

        let (finished_tx, finished_rx) = tokio::sync::oneshot::channel();

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .context("Creating runtime")?;
        let cancel = Arc::new(Notify::new());
        let cancel_clone = cancel.clone();
        let service_handle = std::thread::spawn(move || {
            runtime.block_on(async {
                for on_serve in self.on_serve {
                    if let Err(e) = on_serve().await {
                        let _ = finished_tx.send(Err(e).context("Calling on_serve API"));
                        return;
                    }
                }
                tokio::select! {
                    result = axum::serve(
                        listener,
                        router
                            .layer(cors)
                            .layer(trace::TraceLayer::new_for_http())
                            .layer(compression::CompressionLayer::new())
                            .layer(decompression::DecompressionLayer::new())
                            .into_make_service_with_connect_info::<SocketAddr>(),
                    ) => {
                        let _ = finished_tx.send(result.context("Serving API"));
                    }
                    _ = cancel_clone.notified() => { }
                }
            });
        });

        tokio::select! {
            result = finished_rx => {
                result.context("Panicked within API service")??;
                unreachable!("API Router terminated successfully")
            }
            _ = async {
                if let Err(e) = tokio::signal::ctrl_c().await {
                    error!("Failed to listen for ctrl-c; Service must be shut down manually: {e:#}");
                    std::future::pending().await
                }
            } => {
                cancel.notify_waiters();
                let _ = service_handle.join();
                Ok(ExitCode::SUCCESS)
            }
            _ = async {
                #[cfg(debug_assertions)]
                hot_reload::REQUESTED_NOTIFY.notified().await;
                #[cfg(not(debug_assertions))]
                std::future::pending::<()>().await;
            } => {
                cancel.notify_waiters();
                let _ = service_handle.join();
                #[cfg(debug_assertions)]
                if let Ok("disable") = std::env::var("HOT_RELOAD").as_deref() {
                    // Do nothing
                } else {
                    hot_reload::reloader().await;
                }
                Ok(ExitCode::SUCCESS)
            }
        }
    }
}

#[derive(Subcommand)]
pub enum Command {
    CreateAdmin {
        username: String,
        #[arg(value_parser = clap::value_parser!(i32).range(0..))]
        user_id: i32,
        permissions: Vec<users::admins::permissions::Permission>,
    },
    Run,
    ResetDB,
}

#[derive(Parser)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[tokio::main(flavor = "current_thread")]
pub async fn init_core<F, Fut>(f: F) -> anyhow::Result<ExitCode>
where
    F: FnOnce(TeachCore) -> Fut,
    Fut: Future<Output = anyhow::Result<TeachCore>>,
{
    let Cli { command } = Cli::parse();
    if !Path::new("teach-config.toml").exists() {
        return Err(anyhow::anyhow!("teach-config.toml does not exist"));
    }
    let config =
        std::fs::read_to_string("teach-config.toml").context("Reading teach-config.toml")?;
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_env("LOG_LEVEL"))
        .init();
    init_db(&config).await?;
    match command {
        Command::CreateAdmin {
            username,
            user_id,
            permissions,
        } => {
            return create_admin(username, user_id.try_into().unwrap(), permissions)
                .await
                .map(|()| ExitCode::SUCCESS);
        }
        Command::Run => {}
        Command::ResetDB => {}
    }

    let builder = get_db().get_database_backend();
    let core = TeachCore {
        router: Router::new(),
        info: FxHashMap::default(),
        schema: Schema::new(builder),
        reset_db: vec![],
        config,
        on_serve: vec![],
        to_drop: vec![],
    };
    let core = auth::add_to_core(core).await;
    let core = users::admins::add_to_core(core);
    let core = users::students::add_to_core(core);
    let core = users::instructors::add_to_core(core);
    let core = siblings::add_to_core(core)?;
    let mut core = f(core).await?;
    let info = std::mem::take(&mut core.info);
    let info = serde_json::to_string(&info).unwrap();
    let info: &_ = Box::leak(info.into_boxed_str());
    core.router = core.router.route(
        "/info",
        get(move || {
            std::future::ready(
                Response::builder()
                    .header("Content-Type", "application/json")
                    .body(Body::from(info))
                    .unwrap(),
            )
        }),
    );

    match command {
        Command::CreateAdmin { .. } => unreachable!(),
        Command::Run => core.serve().await,
        Command::ResetDB => core.reset_db().await,
    }
}

#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a function pointer that accepts a `TeachCore` and returns a future which resolves to `anyhow::Result<TeachCore>`"
)]
pub trait AddToCore<S> {
    fn call(
        self,
        core: TeachCore<S>,
    ) -> impl std::future::Future<Output = anyhow::Result<TeachCore<S>>>;
}

impl<Fut, S> AddToCore<S> for fn(TeachCore<S>) -> Fut
where
    Fut: Future<Output = anyhow::Result<TeachCore<S>>>,
{
    async fn call(self, core: TeachCore<S>) -> anyhow::Result<TeachCore<S>> {
        self(core).await
    }
}

pub mod prelude {
    pub use super::{init_core, AddToCore};
}

#[cfg(debug_assertions)]
mod hot_reload {
    use std::{
        future::Future,
        sync::atomic::{AtomicBool, Ordering},
        task::{Context, Poll},
    };

    pub static UPDATED: AtomicBool = AtomicBool::new(false);
    pub static UPDATED_NOTIFY: Notify = Notify::const_new();
    pub static REQUESTED_NOTIFY: Notify = Notify::const_new();

    use axum::{body::Body, extract::Request, response::Response, routing::Route};
    use notify::{Config, EventKind, PollWatcher, Watcher};
    use tokio::{process::Command, sync::Notify};
    use tower::{Layer, Service};
    use tracing::{error, info};

    pub async fn reloader() {
        loop {
            tracing::warn!("Reloading now");
            let mut child = Command::new("cargo")
                .env("HOT_RELOAD", "disable")
                .args(["run", "--", "run"])
                .kill_on_drop(true)
                .spawn()
                .expect("Reloading failed");
            tokio::select! {
                result = child.wait() => {
                    let status = result.expect("Waiting for child process");
                    if !status.success() {
                        tracing::warn!("Waiting for change before reloading");
                        UPDATED.store(false, Ordering::Relaxed);
                        UPDATED_NOTIFY.notified().await;
                    }
                }
                _ = async {
                    if let Err(e) = tokio::signal::ctrl_c().await {
                        error!("Failed to listen for ctrl-c; Service must be shut down manually: {e:#}");
                        std::future::pending().await
                    }
                } => {
                    Command::new("kill")
                        .args(["-s", "INT", &child.id().expect("Getting child process id").to_string()])
                        .output()
                        .await
                        .expect("Killing child process");
                    break;
                }
            }
        }
    }

    #[derive(Clone)]
    pub struct HotReloadLayer {}

    impl Default for HotReloadLayer {
        fn default() -> Self {
            let mut watcher = PollWatcher::new(
                move |result: Result<notify::Event, notify::Error>| {
                    match result {
                        Ok(event) => match event.kind {
                            EventKind::Modify(_) => {
                                info!("{:?} modified", event.paths[0]);
                            }
                            _ => return,
                        },
                        Err(e) => {
                            error!("Error watching for file changes: {e:#}");
                        }
                    }
                    UPDATED.store(true, Ordering::Relaxed);
                    UPDATED_NOTIFY.notify_waiters();
                },
                Config::default().with_manual_polling(),
            )
            .expect("Creating file watcher");
            let mut path = std::env::current_exe().expect("Getting current executable path");
            path.pop();
            path.pop();
            path.pop();
            path.pop();
            path.pop();
            path.push("teach-tech-core");
            path.push("src");
            if path.exists() && path.is_dir() {
                watcher
                    .watch(&path, notify::RecursiveMode::Recursive)
                    .expect("Watching for file changes");
            }
            path.pop();
            path.pop();
            path.push("teach-tech");
            path.push("src");
            if path.exists() && path.is_dir() {
                watcher
                    .watch(&path, notify::RecursiveMode::Recursive)
                    .expect("Watching for file changes");
            }
            std::thread::spawn(move || loop {
                if !UPDATED.load(Ordering::Relaxed) {
                    if let Err(e) = watcher.poll() {
                        error!("Error polling for file changes: {e:#}");
                    }
                }
                std::thread::sleep(std::time::Duration::from_secs(2));
            });
            info!("Watching for file changes in {path:?}");

            Self {}
        }
    }

    impl Layer<Route> for HotReloadLayer {
        type Service = HotReloadService;

        fn layer(&self, service: Route) -> Self::Service {
            HotReloadService { service }
        }
    }

    #[derive(Clone)]
    pub struct HotReloadService {
        service: Route,
    }

    impl Service<Request> for HotReloadService {
        type Response = <Route as Service<Request>>::Response;
        type Error = <Route as Service<Request>>::Error;
        type Future = impl Future<Output = <<Route as Service<Request>>::Future as Future>::Output>;

        fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Service::<Request>::poll_ready(&mut self.service, cx)
        }

        fn call(&mut self, request: Request<Body>) -> Self::Future {
            let fut = self.service.call(request);
            async move {
                if UPDATED.load(Ordering::Relaxed) {
                    REQUESTED_NOTIFY.notify_waiters();
                    // panic!("{}", std::process::id());
                    Ok(Response::builder().status(503).body(Body::empty()).unwrap())
                } else {
                    fut.await
                }
            }
        }
    }
}
