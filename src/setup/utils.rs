use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::env;
use std::path::Path;
use tokio::fs;

pub fn detect_binary() -> Result<String> {
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

pub async fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.exists()
    {
        fs::create_dir_all(parent)
            .await
            .context("Failed to create parent directories")?;
    }
    Ok(())
}

pub async fn read_json_config(path: &Path) -> Result<Value> {
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

pub async fn write_json_config(path: &Path, value: &Value) -> Result<()> {
    ensure_parent_dir(path).await?;
    let content = serde_json::to_string_pretty(value).context("Failed to serialize config")?;
    fs::write(path, content)
        .await
        .context("Failed to write config file")?;
    Ok(())
}

pub fn merge_mcp_server(config: &mut Value, server_name: &str, server_config: Value) {
    if !config["mcpServers"].is_object() {
        config["mcpServers"] = json!({});
    }
    config["mcpServers"][server_name] = server_config;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_mcp_server() {
        let mut config = json!({});
        let server_config = json!({
            "command": "eidetic",
            "args": ["server"]
        });

        merge_mcp_server(&mut config, "eidetic-server", server_config.clone());

        assert!(config["mcpServers"].is_object());
        assert_eq!(config["mcpServers"]["eidetic-server"]["command"], "eidetic");

        // Merge a second server
        merge_mcp_server(&mut config, "other-server", json!({"command": "other"}));
        assert_eq!(config["mcpServers"]["eidetic-server"]["command"], "eidetic");
        assert_eq!(config["mcpServers"]["other-server"]["command"], "other");
    }

    #[tokio::test]
    async fn test_read_write_json_config() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("config.json");

        // Write config
        let value = json!({"hello": "world"});
        write_json_config(&file_path, &value).await.unwrap();

        // Read config back
        let read_val = read_json_config(&file_path).await.unwrap();
        assert_eq!(read_val["hello"], "world");

        // Read non-existent config should return empty object
        let empty_path = dir.path().join("does_not_exist.json");
        let empty_val = read_json_config(&empty_path).await.unwrap();
        assert_eq!(empty_val, json!({}));
    }
}
