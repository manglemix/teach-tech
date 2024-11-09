#![feature(duration_constructors)]
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

pub mod db;
pub mod users;
pub mod auth;

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
        let core = users::admins::add_to_core(core).await;

        let cors = cors::CorsLayer::new()
            .allow_methods(cors::Any);

        #[cfg(debug_assertions)]
        let cors = cors.allow_origin(cors::Any).allow_headers(cors::Any);
        
        let service = tokio::spawn(
            async move {
                axum::serve(
                    listener,
                    core.router
                        .layer(cors)
                        .layer(trace::TraceLayer::new_for_http())
                        .layer(compression::CompressionLayer::new())
                        .layer(decompression::DecompressionLayer::new())
                        .into_make_service_with_connect_info::<SocketAddr>(),
                )
                .await
                .context("Serving API")
            }
        );
        
        tokio::select! {
            result = service => {
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
        }
    }
}

#[derive(Subcommand)]
pub enum Command {
    CreateAdmin {
        username: String,
    },
    Run
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
    tracing_subscriber::fmt().with_env_filter(EnvFilter::from_env("LOG_LEVEL")).init();
    init_db(&config).await?;
    match command {
        Command::CreateAdmin { username } => {
            return create_admin(username).await.map(|()| ExitCode::SUCCESS);
        }
        Command::Run => {}
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
