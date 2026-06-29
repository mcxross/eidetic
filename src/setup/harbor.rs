use anyhow::{Context, Result};

pub async fn setup_harbor() -> Result<()> {
    println!("=== Eidetic Harbor Backup Setup ===");
    println!();
    println!("You will need your Harbor API key and service private key.");
    println!("These are displayed ONLY ONCE on the Harbor web console.");
    println!("They will be stored securely in your system keychain.");
    println!();

    print!("Harbor API Key: ");
    use std::io::Write;
    std::io::stdout().flush()?;
    let api_key = rpassword::read_password().context("Failed to read API key")?;
    if api_key.trim().is_empty() {
        anyhow::bail!("API key cannot be empty");
    }

    print!("Harbor Service Private Key: ");
    std::io::stdout().flush()?;
    let service_key = rpassword::read_password().context("Failed to read service key")?;
    if service_key.trim().is_empty() {
        anyhow::bail!("Service private key cannot be empty");
    }

    crate::harbor::HarborCredentials::store(api_key.trim(), service_key.trim())?;

    println!();
    println!("✅ Credentials stored securely in system keychain.");
    println!();
    println!("⚠️  These credentials CANNOT be recovered from Harbor.");
    println!("    They are now stored in your system keychain under 'eidetic-harbor'.");
    println!();
    println!("Next steps:");
    println!("  1. Run `eidetic backup` to create your first backup.");
    println!("     (On first run, it will reserve a Harbor bucket automatically.)");

    Ok(())
}
