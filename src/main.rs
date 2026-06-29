#![allow(dead_code)]

use clap::{Parser, Subcommand};
use rmcp::{ServiceExt, transport::stdio};
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod auth;
mod config;
mod harbor;
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
    Setup {
        agent: String,
    },
    Update,
    Info,
    /// Back up the SQLite database to Harbor
    Backup,
    /// Restore a SQLite database backup from Harbor
    Restore {
        /// Day of the week to restore (e.g., "monday", "tuesday")
        day: String,
    },
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
        Commands::Tui => run_tui(backend, storage_path, auth_config, config).await,
        Commands::Setup { agent } => setup::run(&agent).await,
        Commands::Update => update::update().await,
        Commands::Info => run_info(config).await,
        Commands::Backup => run_backup(config).await,
        Commands::Restore { day } => run_restore(config, &day).await,
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
    config: crate::config::EideticConfig,
) -> anyhow::Result<()> {
    let store = crate::storage::MemoryStore::new(backend, path, auth_config).await?;
    let storage: Arc<dyn crate::storage::Storage> = store.storage();
    crate::tui::run(storage, config).await
}

async fn run_backup(mut config: config::EideticConfig) -> anyhow::Result<()> {
    println!("=== Eidetic Backup ===");

    let credentials = crate::harbor::HarborCredentials::load()?;

    // Get or create bucket
    let harbor_config = config.harbor.get_or_insert_with(Default::default);

    let bucket_id = if let Some(ref id) = harbor_config.bucket_id {
        println!("Using existing bucket: {}", id);
        id.clone()
    } else {
        println!("No bucket configured. Reserving a new Harbor bucket...");
        let harbor =
            harbor_core::client::HarborClient::new(harbor_core::client::HarborClientOptions {
                api_key: credentials.api_key.clone(),
                ..Default::default()
            });

        let spaces = harbor.list_spaces().await?;
        let space = spaces
            .first()
            .ok_or_else(|| anyhow::anyhow!("No Harbor spaces found for this API key"))?;
        println!("  Space: {}", space.id);

        let reserved = harbor.reserve_bucket(&space.id, "eidetic-backup").await?;
        println!("  Reserved bucket: {}", reserved.bucket_id);

        use fastcrypto::traits::ToFromBytes;
        // Sign with service private key
        let keypair = if credentials.service_private_key.starts_with("suiprivkey") {
            use bech32::FromBase32;
            let (_, data, _) =
                bech32::decode(&credentials.service_private_key).expect("Invalid bech32");
            let bytes = Vec::<u8>::from_base32(&data).expect("Invalid base32");
            let secret_key = bytes[1..].to_vec();
            fastcrypto::ed25519::Ed25519KeyPair::from_bytes(&secret_key)
                .expect("Invalid ed25519 key")
        } else {
            let privkey_bytes = base64::Engine::decode(
                &base64::engine::general_purpose::STANDARD,
                &credentials.service_private_key,
            )
            .expect("Invalid base64");
            fastcrypto::ed25519::Ed25519KeyPair::from_bytes(&privkey_bytes)
                .expect("Invalid ed25519 key")
        };

        let signature = harbor_core::seal::sign_reserve_bytes(&keypair, &reserved.bytes)?;
        let finalized = harbor
            .finalize_bucket(&reserved.bucket_id, &signature)
            .await?;
        println!("  Finalized. Seal policy: {}", finalized.seal_policy_id);

        harbor_config.space_id = Some(space.id.clone());
        harbor_config.bucket_id = Some(reserved.bucket_id.clone());
        harbor_config.seal_policy_id = Some(finalized.seal_policy_id);
        config.save().await?;
        println!("  Saved Harbor config.");

        reserved.bucket_id
    };

    let manager = crate::harbor::HarborBackupManager::new(&credentials, bucket_id);

    let storage_path = config
        .storage_path
        .clone()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(crate::storage::get_storage_path);
    let db_path = storage_path.join("eidetic.db");

    if !db_path.exists() {
        anyhow::bail!(
            "No database found at {}. Nothing to back up.",
            db_path.display()
        );
    }

    println!("\nCreating backup...");
    let file_name = manager.backup(&db_path).await?;
    println!("✅ Backup uploaded as '{}'", file_name);

    // Update last_backup_at
    if let Some(ref mut hc) = config.harbor {
        hc.last_backup_at = Some(chrono::Utc::now().to_rfc3339());
    }
    config.save().await?;

    Ok(())
}

async fn run_restore(config: config::EideticConfig, day: &str) -> anyhow::Result<()> {
    println!("=== Eidetic Restore ===");
    println!();

    let harbor_config = config.harbor.as_ref().ok_or_else(|| {
        anyhow::anyhow!("No Harbor backup configured. Run 'eidetic backup' first.")
    })?;
    let bucket_id = harbor_config
        .bucket_id
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No bucket ID found in config."))?;

    let credentials = crate::harbor::HarborCredentials::load()?;
    let manager = crate::harbor::HarborBackupManager::new(&credentials, bucket_id.clone());

    let target_name = format!("backup_{}.enc", day.to_lowercase());
    println!("Looking for backup: {}", target_name);

    let backups = manager.list_backups().await?;
    let backup = backups
        .iter()
        .find(|f| f.name.as_deref() == Some(&target_name))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No backup found for '{}'. Available: {}",
                day,
                backups
                    .iter()
                    .filter_map(|f| f.name.as_deref())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;

    println!(
        "Found backup: {} ({}B)",
        backup.id,
        backup.size.unwrap_or(0)
    );
    println!();
    println!("⚠️  WARNING: This will OVERWRITE your current memory database.");
    println!("   Type YES to confirm:");

    let mut confirmation = String::new();
    std::io::stdin().read_line(&mut confirmation)?;
    if confirmation.trim() != "YES" {
        println!("Restore cancelled.");
        return Ok(());
    }

    println!("\nDownloading and verifying...");
    let db_bytes = manager.download_backup(&backup.id).await?;

    let storage_path = config
        .storage_path
        .map(std::path::PathBuf::from)
        .unwrap_or_else(crate::storage::get_storage_path);
    let db_path = storage_path.join("eidetic.db");

    // Write to temp file first
    let temp_path = db_path.with_extension("restore.tmp");
    tokio::fs::write(&temp_path, &db_bytes).await?;

    // Run integrity check
    println!("Running integrity check...");
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(&temp_path)
                .read_only(true),
        )
        .await?;

    let result: (String,) = sqlx::query_as("PRAGMA integrity_check")
        .fetch_one(&pool)
        .await?;
    pool.close().await;

    if result.0 != "ok" {
        let _ = tokio::fs::remove_file(&temp_path).await;
        anyhow::bail!("Integrity check failed: {}", result.0);
    }

    // Overwrite
    println!("Replacing database...");
    let wal_path = db_path.with_extension("db-wal");
    let shm_path = db_path.with_extension("db-shm");
    let _ = tokio::fs::remove_file(&wal_path).await;
    let _ = tokio::fs::remove_file(&shm_path).await;
    tokio::fs::rename(&temp_path, &db_path).await?;

    println!("✅ Database restored successfully from '{}' backup.", day);
    println!("   Restart the Eidetic server to use the restored data.");

    Ok(())
}
