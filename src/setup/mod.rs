use anyhow::Result;

pub mod harbor;
pub mod mcp_clients;
pub mod memwal;
pub mod utils;

pub async fn run(agent: &str) -> Result<()> {
    match agent.to_lowercase().as_str() {
        "claude" => mcp_clients::setup_claude().await?,
        "claude-desktop" => mcp_clients::setup_claude_desktop().await?,
        "gemini-cli" => mcp_clients::setup_gemini_cli().await?,
        "opencode" => mcp_clients::setup_opencode().await?,
        "codex" => mcp_clients::setup_codex().await?,
        "cursor" | "pi" => mcp_clients::setup_cursor().await?,
        "vscode" => mcp_clients::setup_vscode().await?,
        "memwal" => return memwal::setup_memwal().await,
        "harbor" => return harbor::setup_harbor().await,
        _ => {
            println!(
                "Unknown setup target '{}'. Supported targets: memwal, harbor, claude, claude-desktop, gemini-cli, opencode, codex, cursor (pi), vscode",
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
