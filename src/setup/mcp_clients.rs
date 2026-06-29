use anyhow::{Context, Result};
use serde_json::json;
use std::env;
use tokio::fs;

use crate::setup::utils::{
    detect_binary, ensure_parent_dir, merge_mcp_server, read_json_config, write_json_config,
};

pub async fn setup_claude() -> Result<()> {
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

pub async fn setup_claude_desktop() -> Result<()> {
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

pub async fn setup_gemini_cli() -> Result<()> {
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

pub async fn setup_opencode() -> Result<()> {
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

pub async fn setup_cursor() -> Result<()> {
    let bin = detect_binary()?;
    let cursor_dir = dirs::home_dir()
        .context("No home directory")?
        .join(".cursor");

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

pub async fn setup_vscode() -> Result<()> {
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

pub async fn setup_codex() -> Result<()> {
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

    if doc["mcp"].get("eidetic").is_none() {
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
