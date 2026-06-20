#![allow(dead_code)]

use clap::{Parser, Subcommand};
use rmcp::{ServiceExt, transport::stdio};
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod auth;
mod memory;
mod server;
mod setup;
mod storage;
mod tools;
mod tui;

use server::EideticServer;

#[derive(Parser, Debug)]
#[command(name = "eidetic", author, version, about = "Eidetic — Agentic Memory MCP Server", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(
        short,
        long,
        env = "EIDETIC_STORAGE_BACKEND",
        default_value = "sqlite",
        global = true
    )]
    pub storage_backend: String,

    #[arg(short = 'p', long, env = "EIDETIC_STORAGE_PATH", global = true)]
    pub storage_path: Option<String>,

    #[arg(long, env = "EIDETIC_MEMWAL_ACCOUNT_ID", global = true)]
    pub memwal_account_id: Option<String>,

    #[arg(long, env = "EIDETIC_MEMWAL_REGISTRY_ID", global = true)]
    pub memwal_registry_id: Option<String>,

    #[arg(long, env = "EIDETIC_MEMWAL_SERVER_URL", global = true)]
    pub memwal_server_url: Option<String>,

    #[arg(long, env = "EIDETIC_MEMWAL_RELAYER_CONFIG_URL", global = true)]
    pub memwal_relayer_config_url: Option<String>,

    #[arg(long, env = "EIDETIC_MEMWAL_NAMESPACE", global = true)]
    pub memwal_namespace: Option<String>,

    #[arg(long, env = "EIDETIC_MEMWAL_DELEGATE_LABEL", global = true)]
    pub memwal_delegate_label: Option<String>,

    #[arg(long, env = "EIDETIC_SUI_CONFIG_DIR", global = true)]
    pub sui_config_dir: Option<String>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Serve,
    Tui,
    Setup { agent: String },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let auth_config = auth_config_from_cli(&cli);

    match cli.command.unwrap_or(Commands::Serve) {
        Commands::Serve => run_server(cli.storage_backend, cli.storage_path, auth_config).await,
        Commands::Tui => run_tui(cli.storage_backend, cli.storage_path, auth_config).await,
        Commands::Setup { agent } => setup::run(&agent).await,
    }
}

fn auth_config_from_cli(cli: &Cli) -> crate::auth::MemwalAuthConfig {
    crate::auth::MemwalAuthConfig {
        account_id: cli.memwal_account_id.clone(),
        registry_id: cli.memwal_registry_id.clone(),
        server_url: cli.memwal_server_url.clone(),
        relayer_config_url: cli.memwal_relayer_config_url.clone(),
        namespace: cli.memwal_namespace.clone(),
        delegate_label: cli.memwal_delegate_label.clone(),
        sui_config_dir: cli.sui_config_dir.as_ref().map(std::path::PathBuf::from),
    }
}

async fn run_server(
    backend: String,
    path: Option<String>,
    auth_config: crate::auth::MemwalAuthConfig,
) -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting Eidetic MCP Server...");

    let store = crate::storage::MemoryStore::new(backend, path, auth_config).await?;
    let server = EideticServer::new(store);

    let service = server.serve(stdio()).await?;

    tracing::info!("Server initialized, waiting for connections...");

    let quit_reason = service.waiting().await?;

    tracing::info!("Server shutting down: {:?}", quit_reason);
    Ok(())
}

async fn run_tui(
    backend: String,
    path: Option<String>,
    auth_config: crate::auth::MemwalAuthConfig,
) -> anyhow::Result<()> {
    let store = crate::storage::MemoryStore::new(backend, path, auth_config).await?;
    let storage: Arc<dyn crate::storage::Storage> = store.storage();
    crate::tui::run(storage).await
}
