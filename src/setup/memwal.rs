use anyhow::Result;

pub async fn setup_memwal() -> Result<()> {
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

    if !has_sui_config && !crate::auth::KeychainManager::is_configured() {
        println!(
            "No Sui config or private key found. Generating a new Ed25519 signer for Memwal..."
        );
        let signer = memwal_core::Ed25519Signer::generate()
            .map_err(|e| anyhow::anyhow!("Failed to generate Ed25519 signer: {}", e))?;
        let suiprivkey = signer
            .to_suiprivkey()
            .map_err(|e| anyhow::anyhow!("Failed to format suiprivkey: {}", e))?;
        let _ = crate::auth::KeychainManager::store_private_key(&suiprivkey);
        config.storage_backend = Some("memwal".to_string());
        config.save().await?;
        println!("Generated new private key and saved to config.");
    }

    let mut auth_config = crate::auth::MemwalAuthConfig {
        account_id: config.memwal_account_id.clone(),
        registry_id: config.memwal_registry_id.clone(),
        server_url: config.memwal_server_url.clone(),
        relayer_config_url: config.memwal_relayer_config_url.clone(),
        namespace: config.memwal_namespace.clone(),
        delegate_label: config.memwal_delegate_label.clone(),
        sui_config_dir: config.sui_config_dir.clone(),
    };
    let temp_auth = crate::auth::AuthManager::new(auth_config.clone()).await?;
    let snapshot_opt = temp_auth.config_snapshot().await.ok();

    // Try to get the address
    let address = if let Ok(suiprivkey) = crate::auth::KeychainManager::load_private_key() {
        use memwal_core::MemWalSigner;
        let signer = memwal_core::Ed25519Signer::from_suiprivkey(&suiprivkey)
            .map_err(|e| anyhow::anyhow!("Failed to parse private key: {}", e))?;
        let addr = signer
            .address()
            .map_err(|e| anyhow::anyhow!("Failed to derive address: {}", e))?;
        format!("0x{}", hex::encode(addr.into_inner()))
    } else if let Some(active_addr) = snapshot_opt
        .as_ref()
        .and_then(|s| s.selected_address.clone())
    {
        active_addr
    } else {
        "your default Sui address".to_string()
    };

    println!("\n=== Memwal Setup ===");
    if config.memwal_account_id.is_some() {
        println!("Account ID found in local config. Validating account and delegate keys...");
    } else {
        println!("We will now connect to the Memwal registry.");
        println!("Since there can only be one Memwal account per Sui address:");
        println!(" - If an account exists for your address, it will be automatically reused.");
        println!(" - If no account exists, a new Memwal account will be provisioned.");
    }

    let is_mainnet = snapshot_opt
        .as_ref()
        .and_then(|snap| snap.active_env.as_deref())
        == Some("mainnet");
    let network_name = if is_mainnet { "Mainnet" } else { "Testnet" };
    let default_registry = if is_mainnet {
        crate::constants::MAINNET_REGISTRY_ID
    } else {
        crate::constants::TESTNET_REGISTRY_ID
    };
    let default_relayer = if is_mainnet {
        crate::constants::MAINNET_RELAYER_URL
    } else {
        crate::constants::TESTNET_RELAYER_URL
    };

    let mut needs_save = false;

    if config.memwal_server_url.is_none() {
        println!(
            "\nSetting default {} Relayer URL: {}",
            network_name, default_relayer
        );
        config.memwal_server_url = Some(default_relayer.to_string());
        auth_config.server_url = config.memwal_server_url.clone();
        needs_save = true;
    }

    if config.memwal_registry_id.is_none() {
        println!("\nA Memwal Registry ID is required to look up or provision an account.");
        print!(
            "Enter Memwal Registry ID (or press Enter to use default {} registry): ",
            network_name
        );
        use std::io::Write;
        std::io::stdout().flush().unwrap();
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        let trimmed = input.trim();
        if !trimmed.is_empty() {
            config.memwal_registry_id = Some(trimmed.to_string());
        } else {
            println!(
                "Using default {} Registry ID: {}",
                network_name, default_registry
            );
            config.memwal_registry_id = Some(default_registry.to_string());
        }
        auth_config.registry_id = config.memwal_registry_id.clone();
        needs_save = true;
    }

    if config.memwal_relayer_config_url.is_none()
        && let Some(relayer) = &config.memwal_server_url
    {
        let config_url = format!("{}/config", relayer);
        println!("Setting default Relayer Config URL: {}", config_url);
        config.memwal_relayer_config_url = Some(config_url);
        auth_config.relayer_config_url = config.memwal_relayer_config_url.clone();
        needs_save = true;
    }

    if config.memwal_namespace.is_none() {
        config.memwal_namespace = Some("eidetic".to_string());
        auth_config.namespace = config.memwal_namespace.clone();
        needs_save = true;
    }

    if config.memwal_delegate_label.is_none() {
        let pool = [
            "hyper-nova",
            "stellar-wind",
            "nebula-drift",
            "cosmic-ray",
            "quantum-flux",
        ];
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let name = pool[(ts as usize) % pool.len()].to_string();
        config.memwal_delegate_label = Some(name);
        auth_config.delegate_label = config.memwal_delegate_label.clone();
        needs_save = true;
    }

    if needs_save {
        config.save().await.unwrap();
    }

    let auth_manager = crate::auth::AuthManager::new(auth_config).await?;

    // Check if account has gas using Sui SDK before attempting to provision
    let mut has_gas = false;
    if let Ok(snap) = auth_manager.config_snapshot().await
        && let Some(rpc_url) = snap.active_rpc.as_deref()
    {
        use std::str::FromStr;
        use sui_sdk::SuiClientBuilder;
        use sui_sdk::types::base_types::SuiAddress;

        if let Ok(parsed_addr) = SuiAddress::from_str(&address)
            && let Ok(sui_client) = SuiClientBuilder::default().build(rpc_url).await
            && let Ok(balance) = sui_client
                .coin_read_api()
                .get_balance(parsed_addr, None)
                .await
        {
            // The balance is returned in MIST. Any non-zero amount means we might have enough,
            // or at least we are funded. If it's truly insufficient later, the loop handles it.
            if balance.total_balance > 0 {
                has_gas = true;
            }
        }
    }

    if !has_gas {
        println!("\nA small amount of SUI gas is required for this process.");
        println!("Address to fund: {}", address);
        println!("Faucet: https://faucet.sui.io/");
        println!("\nWaiting for funding... Checking every 5 seconds.");
    } else {
        println!("Sui account is funded. Proceeding with Memwal lookup/provisioning...");
    }

    loop {
        match auth_manager.memwal_client().await {
            Ok(_client) => {
                let snap = auth_manager.config_snapshot().await?;
                if config.memwal_account_id.is_none() {
                    println!(
                        "\nSuccess! Memwal account successfully provisioned (or reused from registry)."
                    );
                    if let Some(account_id) = snap.memwal_account_id {
                        config.memwal_account_id = Some(account_id);
                        config.save().await?;
                        println!("Saved Account ID to config.json.");
                    }
                } else {
                    println!("\nSuccess! Memwal account verified and ready for use.");
                }
                break;
            }
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("Insufficient gas")
                    || msg.contains("GasBalanceTooLow")
                    || msg.contains("faucet.sui.io")
                {
                    if has_gas {
                        // In case we wrongly thought we had enough gas
                        println!("\nProvisioning requires a larger amount of SUI gas.");
                        println!("Address to fund: {}", address);
                        println!("Faucet: https://faucet.sui.io/");
                        println!("\nWaiting for funding... Checking every 5 seconds.");
                        has_gas = false;
                    } else {
                        println!("Still waiting for gas at {} ... (retrying in 5s)", address);
                    }
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
