#![allow(dead_code)]

use clap::{Parser, Subcommand};
use rmcp::{ServiceExt, transport::stdio};
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

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
}

#[derive(Subcommand, Debug)]
enum Commands {
    Serve,
    Tui,
    Setup {
        agent: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command.unwrap_or(Commands::Serve) {
        Commands::Serve => run_server(cli.storage_backend, cli.storage_path).await,
        Commands::Tui => run_tui(cli.storage_backend, cli.storage_path).await,
        Commands::Setup { agent } => setup::run(&agent).await,
    }
}

async fn run_server(backend: String, path: Option<String>) -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting Eidetic MCP Server...");

    let store = crate::storage::MemoryStore::new(backend, path).await?;
    let server = EideticServer::new(store);

    let service = server.serve(stdio()).await?;

    tracing::info!("Server initialized, waiting for connections...");

    let quit_reason = service.waiting().await?;

    tracing::info!("Server shutting down: {:?}", quit_reason);
    Ok(())
}

async fn run_tui(backend: String, path: Option<String>) -> anyhow::Result<()> {
    let store = crate::storage::MemoryStore::new(backend, path).await?;
    let storage: Arc<dyn crate::storage::Storage> = store.storage();
    crate::tui::run(storage).await
}
