#![allow(dead_code)]

use clap::{Parser, Subcommand};
use rmcp::{ServiceExt, transport::stdio};
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod auth;
mod config;
mod memory;
mod server;
mod setup;
mod storage;
mod tools;
mod tui;
mod update;

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

    #[arg(long, env = "EIDETIC_PRIVATE_KEY", global = true)]
    pub private_key: Option<String>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Serve,
    Tui,
    Setup { agent: String },
    Update,
    Info,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // 1. Load persisted configuration
    let mut config = config::EideticConfig::load().await?;

    // 2. Merge CLI arguments (CLI takes precedence)
    if cli.storage_backend != "sqlite" || config.storage_backend.is_none() {
        config.storage_backend = Some(cli.storage_backend.clone());
    }
    if cli.storage_path.is_some() {
        config.storage_path = cli.storage_path.clone();
    }
    if cli.memwal_account_id.is_some() {
        config.memwal_account_id = cli.memwal_account_id.clone();
    }
    if cli.memwal_registry_id.is_some() {
        config.memwal_registry_id = cli.memwal_registry_id.clone();
    }
    if cli.memwal_server_url.is_some() {
        config.memwal_server_url = cli.memwal_server_url.clone();
    }
    if cli.memwal_relayer_config_url.is_some() {
        config.memwal_relayer_config_url = cli.memwal_relayer_config_url.clone();
    }
    if cli.memwal_namespace.is_some() {
        config.memwal_namespace = cli.memwal_namespace.clone();
    }
    if cli.memwal_delegate_label.is_some() {
        config.memwal_delegate_label = cli.memwal_delegate_label.clone();
    }
    if cli.sui_config_dir.is_some() {
        config.sui_config_dir = cli.sui_config_dir.clone().map(std::path::PathBuf::from);
    }
    if cli.private_key.is_some() {
        config.private_key = cli.private_key.clone();
    }

    let backend = config
        .storage_backend
        .clone()
        .unwrap_or_else(|| "sqlite".to_string());

    // 3. Auto-generate private key if no Sui config and no key provided (ONLY for memwal)
    let has_sui_config = config
        .sui_config_dir
        .clone()
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_default()
                .join(".sui")
                .join("sui_config")
        })
        .join("client.yaml")
        .exists();

    let mut config_changed = false;
    if backend == "memwal" && !has_sui_config && config.private_key.is_none() {
        tracing::info!(
            "No Sui config or private key found. Generating a new one for Memwal backend..."
        );
        let signer = memwal_core::Ed25519Signer::generate()
            .map_err(|e| anyhow::anyhow!("Failed to generate Ed25519 signer: {}", e))?;
        let suiprivkey = signer
            .to_suiprivkey()
            .map_err(|e| anyhow::anyhow!("Failed to format suiprivkey: {}", e))?;
        config.private_key = Some(suiprivkey);
        config_changed = true;
    }

    let auth_config = auth_config_from_config(&config);
    let storage_path = config.storage_path.clone();

    // 4. Run the subcommand

    match cli.command.unwrap_or(Commands::Serve) {
        Commands::Serve => {
            run_server(
                backend,
                storage_path,
                auth_config,
                config.clone(),
                config_changed,
            )
            .await
        }
        Commands::Tui => run_tui(backend, storage_path, auth_config).await,
        Commands::Setup { agent } => setup::run(&agent).await,
        Commands::Update => update::update().await,
        Commands::Info => run_info(config).await,
    }
}

async fn run_info(config: config::EideticConfig) -> anyhow::Result<()> {
    println!("=== Eidetic Configuration Info ===\n");

    println!(
        "Storage Backend: {}",
        config.storage_backend.as_deref().unwrap_or("sqlite")
    );
    if let Some(path) = &config.storage_path {
        println!("Storage Path: {}", path);
    } else {
        println!("Storage Path: Default (~/.eidetic/storage)");
    }
    println!(
        "Memwal Account ID: {}",
        config
            .memwal_account_id
            .as_deref()
            .unwrap_or("Not provisioned")
    );
    println!(
        "Memwal Registry ID: {}",
        config.memwal_registry_id.as_deref().unwrap_or("Default")
    );

    println!("\n=== Sui Identity ===");

    if let Some(suiprivkey) = &config.private_key {
        use memwal_core::MemWalSigner;
        let signer = memwal_core::Ed25519Signer::from_suiprivkey(suiprivkey)
            .map_err(|e| anyhow::anyhow!("Failed to parse configured private key: {}", e))?;

        let address = signer
            .address()
            .map_err(|e| anyhow::anyhow!("Failed to derive address: {}", e))?;

        let address_hex = format!("0x{}", hex::encode(address.into_inner()));
        println!("Active Address: {}", address_hex);
        println!("Source: config.json (private_key)");
        println!(
            "\nIf you are using Memwal, please ensure this address is funded with SUI for gas fees."
        );
        println!("Sui Testnet faucet: https://faucet.sui.io/");
    } else if let Some(sui_dir) = &config.sui_config_dir {
        println!("Source: Custom Sui Config Dir ({})", sui_dir.display());
        println!("(Run `sui client active-address` to check your configured address)");
    } else {
        println!("Source: Default Sui Config (~/.sui/sui_config)");
        println!("(Run `sui client active-address` to check your configured address)");
    }

    Ok(())
}

fn auth_config_from_config(config: &config::EideticConfig) -> crate::auth::MemwalAuthConfig {
    crate::auth::MemwalAuthConfig {
        account_id: config.memwal_account_id.clone(),
        registry_id: config.memwal_registry_id.clone(),
        server_url: config.memwal_server_url.clone(),
        relayer_config_url: config.memwal_relayer_config_url.clone(),
        namespace: config.memwal_namespace.clone(),
        delegate_label: config.memwal_delegate_label.clone(),
        sui_config_dir: config.sui_config_dir.clone(),
        private_key: config.private_key.clone(),
    }
}

async fn run_server(
    backend: String,
    path: Option<String>,
    auth_config: crate::auth::MemwalAuthConfig,
    mut eidetic_config: config::EideticConfig,
    mut config_changed: bool,
) -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting Eidetic MCP Server...");

    let store = crate::storage::MemoryStore::new(backend, path, auth_config).await?;

    // If memwal provisioned a new account, save it
    if let Some(auth_mgr) = store.auth_manager()
        && let Ok(snap) = auth_mgr.config_snapshot().await
        && eidetic_config.memwal_account_id != snap.memwal_account_id
        && snap.memwal_account_id.is_some()
    {
        eidetic_config.memwal_account_id = snap.memwal_account_id;
        config_changed = true;
    }

    if config_changed {
        if let Err(e) = eidetic_config.save().await {
            tracing::warn!("Failed to save Eidetic configuration: {}", e);
        } else {
            tracing::info!(
                "Saved Eidetic configuration to {:?}",
                config::EideticConfig::config_path().unwrap_or_default()
            );
        }
    }

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
