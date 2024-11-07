use std::{
    future::Future,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::Path,
    process::ExitCode,
};

use anyhow::Context;
use axum::Router;
use db::init_db;
use serde::{Deserialize, Serialize};
use tracing::error;

pub mod db;

#[derive(Debug, Clone, Deserialize, Serialize)]
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
        init_db(self.get_config_str()).await?;

        let api_config: ApiConfig =
            toml::from_str(self.get_config_str()).context("Parsing teach-config.toml")?;

        let listener = tokio::net::TcpListener::bind(api_config.server_address)
            .await
            .with_context(|| format!("Binding to {}", api_config.server_address))?;
        
        let service = tokio::spawn(
            async move {
                axum::serve(
                    listener,
                    self.router
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
                    error!("Failed to listen for ctrl-c; Service must be shut down manually: {e}");
                    std::future::pending().await
                }
            } => {
                Ok(ExitCode::SUCCESS)
            }
        }
    }
}

#[tokio::main]
pub async fn init_core<F, Fut>(f: F) -> anyhow::Result<ExitCode>
where
    F: FnOnce(TeachCore) -> Fut,
    Fut: Future<Output = anyhow::Result<ExitCode>>,
{
    if !Path::new("teach-config.toml").exists() {
        return Err(anyhow::anyhow!("teach-config.toml does not exist"));
    }
    let config =
        std::fs::read_to_string("teach-config.toml").context("Reading teach-config.toml")?;
    // Check if the config is valid
    toml::from_str::<toml::Value>(&config).context("Validating teach-config.toml")?;
    let core = TeachCore {
        router: Router::new(),
        config,
    };
    tracing_subscriber::fmt().init();
    f(core).await
}

pub mod prelude {
    pub use super::init_core;
}
