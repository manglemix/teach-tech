#![feature(duration_constructors)]
#![feature(impl_trait_in_assoc_type)]

use std::{
    future::Future,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::Path,
    process::ExitCode,
};

use anyhow::Context;
use axum::Router;
use clap::{Parser, Subcommand};
use db::init_db;
use serde::Deserialize;
use tower_http::{compression, cors, decompression, trace};
use tracing::error;
use tracing_subscriber::EnvFilter;
use users::admins::create_admin;

pub mod auth;
pub mod db;
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
    config: String,
}

impl<S> TeachCore<S> {
    pub fn get_config_str(&self) -> &str {
        &self.config
    }

    pub fn modify_router<T>(self, f: impl FnOnce(Router<S>) -> Router<T>) -> TeachCore<T> {
        TeachCore {
            router: f(self.router),
            config: self.config,
        }
    }
}

impl TeachCore<()> {
    pub async fn serve(self) -> anyhow::Result<ExitCode> {
        let api_config: ApiConfig =
            toml::from_str(self.get_config_str()).context("Parsing teach-config.toml")?;

        let listener = tokio::net::TcpListener::bind(api_config.server_address)
            .await
            .with_context(|| format!("Binding to {}", api_config.server_address))?;

        let core = auth::add_to_core(self).await;
        let core = users::admins::add_to_core(core);
        let core = users::students::add_to_core(core);

        let cors = cors::CorsLayer::new().allow_methods(cors::Any);

        #[cfg(debug_assertions)]
        let cors = cors.allow_origin(cors::Any).allow_headers(cors::Any);
        let router = core.router;
        #[cfg(debug_assertions)]
        let router = router.layer(hot_reload::HotReloadLayer::default());

        let (finished_tx, finished_rx) = tokio::sync::oneshot::channel();

        let service = tokio::spawn(async move {
            let result = axum::serve(
                listener,
                router
                    .layer(cors)
                    .layer(trace::TraceLayer::new_for_http())
                    .layer(compression::CompressionLayer::new())
                    .layer(decompression::DecompressionLayer::new())
                    .into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            .context("Serving API");
            let _ = finished_tx.send(result);
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
                Ok(ExitCode::SUCCESS)
            }
            _ = async {
                #[cfg(debug_assertions)]
                hot_reload::REQUESTED_NOTIFY.notified().await;
                #[cfg(not(debug_assertions))]
                std::future::pending::<()>().await;
            } => {
                service.abort();
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
    CreateAdmin { username: String },
    Run,
    ResetDB,
}

#[derive(Parser)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[tokio::main]
pub async fn init_core<F, Fut>(f: F) -> anyhow::Result<ExitCode>
where
    F: FnOnce(TeachCore) -> Fut,
    Fut: Future<Output = anyhow::Result<ExitCode>>,
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
        Command::CreateAdmin { username } => {
            return create_admin(username).await.map(|()| ExitCode::SUCCESS);
        }
        Command::Run => {}
        Command::ResetDB => {
            return db::reset_db(&config).await.map(|()| ExitCode::SUCCESS);
        }
    }

    let core = TeachCore {
        router: Router::new(),
        config,
    };
    f(core).await
}

pub mod prelude {
    pub use super::init_core;
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
            path.push("lib.rs");
            watcher
                .watch(&path, notify::RecursiveMode::Recursive)
                .expect("Watching for file changes");
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
                    Ok(Response::builder().status(503).body(Body::empty()).unwrap())
                } else {
                    fut.await
                }
            }
        }
    }
}
