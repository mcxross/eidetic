use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::env;
use std::path::Path;
use tokio::fs;

pub async fn run(agent: &str) -> Result<()> {
    match agent.to_lowercase().as_str() {
        "claude" => setup_claude().await?,
        "claude-desktop" => setup_claude_desktop().await?,
        "gemini-cli" => setup_gemini_cli().await?,
        "opencode" => setup_opencode().await?,
        "codex" => setup_codex().await?,
        "cursor" | "pi" => setup_cursor().await?,
        "vscode" => setup_vscode().await?,
        "memwal" => return setup_memwal().await,
        _ => {
            println!(
                "Unknown setup target '{}'. Supported targets: memwal, claude, claude-desktop, gemini-cli, opencode, codex, cursor (pi), vscode",
                agent
            );
            return Ok(());
        }
    }

    println!(
        "\nSuccess! Eidetic MCP server has been configured for {}.",
        agent
    );
    println!("You may need to restart the agent for the changes to take effect.");
    Ok(())
}

fn detect_binary() -> Result<String> {
    let exe_path = env::current_exe().context("Failed to get current executable path")?;
    let path_str = exe_path.to_string_lossy().to_string();

    if path_str.contains("target/debug") || path_str.contains("target/release") {
        println!(
            "⚠️ Warning: You are registering a binary from a cargo target directory ({}).",
            path_str
        );
        println!("   It is recommended to install Eidetic globally using `cargo install --path .`");
        println!(
            "   and then run `eidetic setup <agent>` so the agent can find the eidetic command globally."
        );
        println!("   Registering anyway...\n");
    }

    Ok(path_str)
}

async fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)
                .await
                .context("Failed to create parent directories")?;
        }
    }
    Ok(())
}

async fn read_json_config(path: &Path) -> Result<Value> {
    if path.exists() {
        let content = fs::read_to_string(path)
            .await
            .context("Failed to read config file")?;
        let mut parsed: Value = serde_json::from_str(&content).unwrap_or_else(|_| json!({}));
        if !parsed.is_object() {
            parsed = json!({});
        }
        Ok(parsed)
    } else {
        Ok(json!({}))
    }
}

async fn write_json_config(path: &Path, value: &Value) -> Result<()> {
    ensure_parent_dir(path).await?;
    let content = serde_json::to_string_pretty(value).context("Failed to serialize config")?;
    fs::write(path, content)
        .await
        .context("Failed to write config file")?;
    Ok(())
}

fn merge_mcp_server(config: &mut Value, server_name: &str, server_config: Value) {
    if !config["mcpServers"].is_object() {
        config["mcpServers"] = json!({});
    }
    config["mcpServers"][server_name] = server_config;
}

async fn setup_claude() -> Result<()> {
    let bin = detect_binary()?;
    let path = dirs::home_dir()
        .context("No home directory found")?
        .join(".claude.json");

    let mut config = read_json_config(&path).await?;
    merge_mcp_server(
        &mut config,
        "eidetic",
        json!({
            "command": bin,
            "args": []
        }),
    );

    write_json_config(&path, &config).await?;
    println!("Updated config at {}", path.display());
    Ok(())
}

async fn setup_claude_desktop() -> Result<()> {
    let bin = detect_binary()?;

    let path = if cfg!(target_os = "macos") {
        dirs::home_dir()
            .context("No home directory")?
            .join("Library/Application Support/Claude/claude_desktop_config.json")
    } else if cfg!(target_os = "windows") {
        dirs::data_local_dir()
            .context("No local data directory")?
            .join("Anthropic/Claude/claude_desktop_config.json")
    } else {
        anyhow::bail!("Claude Desktop configuration location is unknown for this OS.");
    };

    let mut config = read_json_config(&path).await?;
    merge_mcp_server(
        &mut config,
        "eidetic",
        json!({
            "command": bin,
            "args": []
        }),
    );

    write_json_config(&path, &config).await?;
    println!("Updated config at {}", path.display());
    Ok(())
}

async fn setup_gemini_cli() -> Result<()> {
    let bin = detect_binary()?;
    let path = dirs::home_dir()
        .context("No home directory found")?
        .join(".gemini/settings.json");

    let mut config = read_json_config(&path).await?;
    merge_mcp_server(
        &mut config,
        "eidetic",
        json!({
            "command": bin,
            "args": []
        }),
    );

    write_json_config(&path, &config).await?;
    println!("Updated config at {}", path.display());
    Ok(())
}

async fn setup_opencode() -> Result<()> {
    let bin = detect_binary()?;
    let path = dirs::config_dir()
        .context("No config directory found")?
        .join("opencode/opencode.json");

    let mut config = read_json_config(&path).await?;

    if !config["mcp"].is_object() {
        config["mcp"] = json!({});
    }

    config["mcp"]["eidetic"] = json!({
        "type": "local",
        "command": [bin],
        "enabled": true
    });

    write_json_config(&path, &config).await?;
    println!("Updated config at {}", path.display());
    Ok(())
}

async fn setup_cursor() -> Result<()> {
    let bin = detect_binary()?;
    let cursor_dir = if cfg!(target_os = "windows") {
        dirs::home_dir()
            .context("No home directory")?
            .join(".cursor")
    } else {
        dirs::home_dir()
            .context("No home directory")?
            .join(".cursor")
    };

    let path = cursor_dir.join("mcp.json");

    let mut config = read_json_config(&path).await?;
    merge_mcp_server(
        &mut config,
        "eidetic",
        json!({
            "command": bin,
            "args": []
        }),
    );

    write_json_config(&path, &config).await?;
    println!("Updated config at {}", path.display());
    Ok(())
}

async fn setup_vscode() -> Result<()> {
    let bin = detect_binary()?;
    let cwd = env::current_dir().context("Failed to get current directory")?;
    let path = cwd.join(".vscode/mcp.json");

    let mut config = read_json_config(&path).await?;

    if !config["servers"].is_object() {
        config["servers"] = json!({});
    }

    config["servers"]["eidetic"] = json!({
        "command": bin,
        "args": []
    });

    write_json_config(&path, &config).await?;
    println!("Updated config at {}", path.display());
    Ok(())
}

async fn setup_codex() -> Result<()> {
    let bin = detect_binary()?;
    let path = dirs::home_dir()
        .context("No home directory found")?
        .join(".codex/config.toml");
    ensure_parent_dir(&path).await?;

    let mut doc = if path.exists() {
        let content = fs::read_to_string(&path)
            .await
            .context("Failed to read config.toml")?;
        content
            .parse::<toml_edit::DocumentMut>()
            .unwrap_or_else(|_| toml_edit::DocumentMut::new())
    } else {
        toml_edit::DocumentMut::new()
    };

    if !doc.contains_key("mcp") {
        doc["mcp"] = toml_edit::Item::Table(toml_edit::Table::new());
    }

    if !doc["mcp"].get("eidetic").is_some() {
        let mut eidetic_table = toml_edit::Table::new();
        eidetic_table.insert("command", toml_edit::value(bin.clone()));
        let args = toml_edit::Array::new();
        eidetic_table.insert(
            "args",
            toml_edit::Item::Value(toml_edit::Value::Array(args)),
        );
        doc["mcp"]["eidetic"] = toml_edit::Item::Table(eidetic_table);
    } else {
        doc["mcp"]["eidetic"]["command"] = toml_edit::value(bin.clone());
    }

    fs::write(&path, doc.to_string())
        .await
        .context("Failed to write config.toml")?;
    println!("Updated config at {}", path.display());
    Ok(())
}

async fn setup_memwal() -> Result<()> {
    let mut config = crate::config::EideticConfig::load().await?;

    // Auto-generate key if needed
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

    if !has_sui_config && config.private_key.is_none() {
        println!(
            "No Sui config or private key found. Generating a new Ed25519 signer for Memwal..."
        );
        let signer = memwal_core::Ed25519Signer::generate()
            .map_err(|e| anyhow::anyhow!("Failed to generate Ed25519 signer: {}", e))?;
        let suiprivkey = signer
            .to_suiprivkey()
            .map_err(|e| anyhow::anyhow!("Failed to format suiprivkey: {}", e))?;
        config.private_key = Some(suiprivkey);
        config.storage_backend = Some("memwal".to_string());
        config.save().await?;
        println!("Generated new private key and saved to config.");
    }

    // Try to get the address
    let address = if let Some(suiprivkey) = &config.private_key {
        use memwal_core::MemWalSigner;
        let signer = memwal_core::Ed25519Signer::from_suiprivkey(suiprivkey)
            .map_err(|e| anyhow::anyhow!("Failed to parse private key: {}", e))?;
        let addr = signer
            .address()
            .map_err(|e| anyhow::anyhow!("Failed to derive address: {}", e))?;
        format!("0x{}", hex::encode(addr.into_inner()))
    } else {
        println!("Using default Sui config for Memwal provisioning.");
        "your default Sui address".to_string()
    };

    println!("\n=== Memwal Setup ===");
    println!("We are about to attempt to provision a Memwal account.");
    println!("This requires a small amount of SUI gas on the Testnet.");
    println!("Address to fund: {}", address);
    println!("Faucet: https://faucet.sui.io/");

    println!("\nWaiting for funding... Checking every 5 seconds.");

    // Build the AuthManager to attempt provisioning
    let auth_config = crate::auth::MemwalAuthConfig {
        account_id: config.memwal_account_id.clone(),
        registry_id: config.memwal_registry_id.clone(),
        server_url: config.memwal_server_url.clone(),
        relayer_config_url: config.memwal_relayer_config_url.clone(),
        namespace: config.memwal_namespace.clone(),
        delegate_label: config.memwal_delegate_label.clone(),
        sui_config_dir: config.sui_config_dir.clone(),
        private_key: config.private_key.clone(),
    };

    let auth_manager = crate::auth::AuthManager::new(auth_config).await?;

    loop {
        match auth_manager.memwal_client().await {
            Ok(client) => {
                println!("Success! Memwal account provisioned.");
                let snap = auth_manager.config_snapshot().await?;
                if let Some(account_id) = snap.memwal_account_id {
                    config.memwal_account_id = Some(account_id);
                    config.save().await?;
                    println!("Saved new Account ID to config.json.");
                }
                break;
            }
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("Insufficient gas")
                    || msg.contains("GasBalanceTooLow")
                    || msg.contains("faucet.sui.io")
                {
                    println!("Still waiting for gas at {} ... (retrying in 5s)", address);
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                } else {
                    println!("Provisioning failed with an unexpected error: {}", msg);
                    anyhow::bail!("Setup failed.");
                }
            }
        }
    }

    println!("\nMemwal Setup Complete! Eidetic is ready for use.");
    Ok(())
}
