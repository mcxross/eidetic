use std::{path::PathBuf, sync::Arc, time::Duration};

use anyhow::{Context, anyhow};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use memwal_core::{Ed25519Signer, MemWal, MemWalProvisionConfig, MemWalSigner};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use tokio::sync::RwLock;

#[derive(Clone, Debug, Default)]
pub struct MemwalAuthConfig {
    pub account_id: Option<String>,
    pub registry_id: Option<String>,
    pub server_url: Option<String>,
    pub relayer_config_url: Option<String>,
    pub namespace: Option<String>,
    pub delegate_label: Option<String>,
    pub sui_config_dir: Option<PathBuf>,
    pub private_key: Option<String>,
}

impl MemwalAuthConfig {
    pub fn namespace(&self) -> String {
        self.namespace
            .clone()
            .unwrap_or_else(|| "eidetic".to_string())
    }

    fn delegate_label(&self) -> String {
        self.delegate_label
            .clone()
            .unwrap_or_else(|| "eidetic-mcp".to_string())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SuiAccountInfo {
    pub address: String,
    pub alias: Option<String>,
    pub active: bool,
    pub active_env: Option<String>,
    pub key_available: bool,
    pub status: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemwalConfigSnapshot {
    pub backend: String,
    pub selected_address: Option<String>,
    pub selected_alias: Option<String>,
    pub active_env: Option<String>,
    pub active_rpc: Option<String>,
    pub namespace: String,
    pub memwal_account_id: Option<String>,
    pub registry_id: Option<String>,
    pub server_url: Option<String>,
    pub relayer_config_url: Option<String>,
    pub delegate_label: String,
    pub provisioned: bool,
    pub diagnostics: Vec<String>,
}

#[derive(Clone)]
pub struct AuthManager {
    config: MemwalAuthConfig,
    state: Arc<RwLock<AuthState>>,
}

#[derive(Default)]
struct AuthState {
    selected: Option<SelectedAccount>,
    provisioned: Option<ProvisionedClient>,
    diagnostics: Vec<String>,
    sui_state: Option<SuiConfigState>,
}

#[derive(Clone)]
struct SelectedAccount {
    address: String,
    alias: Option<String>,
    suiprivkey: String,
}

#[derive(Clone)]
struct ProvisionedClient {
    address: String,
    alias: Option<String>,
    memwal_account_id: String,
    client: Arc<MemWal>,
}

#[derive(Clone, Debug, Default)]
struct SuiConfigState {
    active_address: Option<String>,
    active_env: Option<String>,
    active_rpc: Option<String>,
    accounts: Vec<ParsedSuiAccount>,
    diagnostics: Vec<String>,
}

#[derive(Clone, Debug)]
struct ParsedSuiAccount {
    address: String,
    alias: Option<String>,
    suiprivkey: Option<String>,
    status: String,
}

impl AuthManager {
    pub async fn new(config: MemwalAuthConfig) -> anyhow::Result<Self> {
        let manager = Self {
            config,
            state: Arc::new(RwLock::new(AuthState::default())),
        };
        manager.reload_sui_config().await?;
        manager.select_default_account().await?;
        Ok(manager)
    }

    pub async fn list_sui_accounts(&self) -> anyhow::Result<Vec<SuiAccountInfo>> {
        self.reload_sui_config().await?;
        let state = self.state.read().await;
        let selected_address = state
            .selected
            .as_ref()
            .map(|selected| selected.address.as_str())
            .or(state
                .sui_state
                .as_ref()
                .and_then(|sui| sui.active_address.as_deref()));

        Ok(state
            .sui_state
            .as_ref()
            .map(|sui| {
                sui.accounts
                    .iter()
                    .map(|account| SuiAccountInfo {
                        address: account.address.clone(),
                        alias: account.alias.clone(),
                        active: selected_address == Some(account.address.as_str()),
                        active_env: sui.active_env.clone(),
                        key_available: account.suiprivkey.is_some(),
                        status: account.status.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    pub async fn select_account(&self, selector: &str) -> anyhow::Result<MemwalConfigSnapshot> {
        self.reload_sui_config().await?;
        let account = {
            let state = self.state.read().await;
            let sui_state = state
                .sui_state
                .as_ref()
                .ok_or_else(|| anyhow!("Sui configuration has not been loaded"))?;
            find_account(sui_state, selector)
                .cloned()
                .ok_or_else(|| anyhow!("Sui account not found for selector `{selector}`"))?
        };

        let suiprivkey = account.suiprivkey.clone().ok_or_else(|| {
            anyhow!("Sui account `{selector}` does not have usable Ed25519 private key material")
        })?;

        {
            let mut state = self.state.write().await;
            state.selected = Some(SelectedAccount {
                address: account.address.clone(),
                alias: account.alias.clone(),
                suiprivkey,
            });
            state.provisioned = None;
        }

        // We don't ensure_memwal_client() here because if a new key was generated,
        // it's guaranteed to fail due to lack of gas. We let it happen lazily on tool calls.
        self.config_snapshot().await
    }

    pub async fn memwal_client(&self) -> anyhow::Result<Arc<MemWal>> {
        self.ensure_memwal_client().await
    }

    pub async fn config_snapshot(&self) -> anyhow::Result<MemwalConfigSnapshot> {
        let state = self.state.read().await;
        let sui_state = state.sui_state.clone().unwrap_or_default();
        let selected = state.selected.clone();
        let provisioned = state.provisioned.clone();
        let mut diagnostics = sui_state.diagnostics;
        diagnostics.extend(state.diagnostics.clone());

        Ok(MemwalConfigSnapshot {
            backend: "memwal".to_string(),
            selected_address: selected.as_ref().map(|s| s.address.clone()),
            selected_alias: selected.as_ref().and_then(|s| s.alias.clone()),
            active_env: sui_state.active_env,
            active_rpc: sui_state.active_rpc,
            namespace: self.config.namespace(),
            memwal_account_id: provisioned.map(|p| p.memwal_account_id),
            registry_id: self.config.registry_id.clone(),
            server_url: self.config.server_url.clone(),
            relayer_config_url: self.config.relayer_config_url.clone(),
            delegate_label: self.config.delegate_label(),
            provisioned: state.provisioned.is_some(),
            diagnostics,
        })
    }

    async fn ensure_memwal_client(&self) -> anyhow::Result<Arc<MemWal>> {
        {
            let state = self.state.read().await;
            if let Some(provisioned) = &state.provisioned {
                return Ok(provisioned.client.clone());
            }
        }

        let selected = {
            let state = self.state.read().await;
            state
                .selected
                .clone()
                .ok_or_else(|| anyhow!("No Sui account is selected for Memwal operations"))?
        };

        let mut config = MemWalProvisionConfig::new(selected.suiprivkey.clone())
            .wallet_suiprivkey(selected.suiprivkey.clone())
            .namespace(self.config.namespace())
            .delegate_label(self.config.delegate_label());
        if let Some(account_id) = &self.config.account_id {
            config = config.account_id(account_id.clone());
        }
        if let Some(registry_id) = &self.config.registry_id {
            config = config.registry_id(registry_id.clone());
        }
        if let Some(server_url) = &self.config.server_url {
            config = config.server_url(server_url.clone());
        }
        if let Some(relayer_config_url) = &self.config.relayer_config_url {
            config = config.relayer_config_url(relayer_config_url.clone());
        }

        let provisioned = tokio::time::timeout(Duration::from_secs(90), MemWal::provision(config))
            .await
            .map_err(|_| anyhow!("Timed out provisioning Memwal client"))?
            .map_err(|error| {
                let msg = error.to_string();
                if msg.contains("GasBalanceTooLow") || msg.contains("insufficient gas") || msg.contains("InsufficientGas") {
                    anyhow!(
                        "Insufficient gas to provision Memwal account.\n\nPlease fund your address: {}\nSui Testnet faucet: https://faucet.sui.io/",
                        selected.address
                    )
                } else {
                    anyhow!("Failed to provision Memwal client: {msg}")
                }
            })?;

        let client = Arc::new(provisioned.memwal().clone());
        let provisioned_client = ProvisionedClient {
            address: selected.address,
            alias: selected.alias,
            memwal_account_id: provisioned.account_id(),
            client: client.clone(),
        };

        let mut state = self.state.write().await;
        state.provisioned = Some(provisioned_client);
        Ok(client)
    }

    async fn select_default_account(&self) -> anyhow::Result<()> {
        if let Some(suiprivkey) = &self.config.private_key {
            let signer = signer_from_keystore_entry(suiprivkey)
                .with_context(|| "Failed to parse provided private_key")?;
            let address = normalize_address(&signer.address()?.to_string());
            let mut state = self.state.write().await;
            state.selected = Some(SelectedAccount {
                address,
                alias: Some("config".to_string()),
                suiprivkey: suiprivkey.clone(),
            });
            return Ok(());
        }

        let default_account = {
            let state = self.state.read().await;
            let Some(sui_state) = &state.sui_state else {
                return Ok(());
            };
            let Some(active_address) = &sui_state.active_address else {
                return Ok(());
            };
            find_account(sui_state, active_address).cloned()
        };

        if let Some(account) = default_account
            && let Some(suiprivkey) = account.suiprivkey
        {
            let mut state = self.state.write().await;
            state.selected = Some(SelectedAccount {
                address: account.address,
                alias: account.alias,
                suiprivkey,
            });
        }
        Ok(())
    }

    async fn reload_sui_config(&self) -> anyhow::Result<()> {
        let sui_state = load_sui_config(self.config.sui_config_dir.clone())?;
        let mut state = self.state.write().await;
        state.sui_state = Some(sui_state);
        Ok(())
    }
}

fn find_account<'a>(state: &'a SuiConfigState, selector: &str) -> Option<&'a ParsedSuiAccount> {
    state.accounts.iter().find(|account| {
        account.address.eq_ignore_ascii_case(selector)
            || account.alias.as_deref() == Some(selector)
            || account.address.strip_prefix("0x").is_some_and(|trimmed| {
                trimmed.eq_ignore_ascii_case(selector.strip_prefix("0x").unwrap_or(selector))
            })
    })
}

fn load_sui_config(config_dir: Option<PathBuf>) -> anyhow::Result<SuiConfigState> {
    let config_dir = config_dir.unwrap_or_else(default_sui_config_dir);
    let client_path = config_dir.join("client.yaml");
    let aliases_path = config_dir.join("sui.aliases");

    if !client_path.is_file() {
        return Ok(SuiConfigState {
            diagnostics: vec![format!(
                "Sui client config not found at {}",
                client_path.display()
            )],
            ..SuiConfigState::default()
        });
    }

    let client_yaml = std::fs::read_to_string(&client_path)
        .with_context(|| format!("Failed to read {}", client_path.display()))?;
    let client_value: serde_yaml::Value = serde_yaml::from_str(&client_yaml)
        .with_context(|| format!("Failed to parse {}", client_path.display()))?;

    let active_address = yaml_string(&client_value, "active_address");
    let active_env = yaml_string(&client_value, "active_env");
    let active_rpc = active_env
        .as_deref()
        .and_then(|alias| active_rpc_for_env(&client_value, alias));
    let keystore_path =
        keystore_path(&client_value).unwrap_or_else(|| config_dir.join("sui.keystore"));
    let aliases = load_aliases(&aliases_path);
    let (mut accounts, mut diagnostics) = load_keystore_accounts(&keystore_path, &aliases)?;

    if accounts.is_empty() {
        diagnostics.push(format!(
            "No usable Sui accounts found in {}",
            keystore_path.display()
        ));
    }
    if let Some(active_address) = &active_address
        && !accounts
            .iter()
            .any(|account| account.address.eq_ignore_ascii_case(active_address))
    {
        accounts.push(ParsedSuiAccount {
            address: normalize_address(active_address),
            alias: None,
            suiprivkey: None,
            status: "active address found in client.yaml but no matching key was found".to_string(),
        });
    }

    Ok(SuiConfigState {
        active_address: active_address.map(|address| normalize_address(&address)),
        active_env,
        active_rpc,
        accounts,
        diagnostics,
    })
}

fn default_sui_config_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".sui")
        .join("sui_config")
}

fn yaml_string(value: &serde_yaml::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
}

fn active_rpc_for_env(value: &serde_yaml::Value, active_env: &str) -> Option<String> {
    value
        .get("envs")?
        .as_sequence()?
        .iter()
        .find(|env| yaml_string(env, "alias").as_deref() == Some(active_env))
        .and_then(|env| yaml_string(env, "rpc"))
}

fn keystore_path(value: &serde_yaml::Value) -> Option<PathBuf> {
    let keystore = value.get("keystore")?;
    if let Some(path) = keystore.as_str() {
        return Some(PathBuf::from(path));
    }
    keystore
        .get("File")
        .and_then(|value| value.as_str())
        .map(PathBuf::from)
}

fn load_aliases(path: &PathBuf) -> Vec<SuiAlias> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    serde_json::from_str(&content).unwrap_or_default()
}

#[derive(Debug, Deserialize)]
struct SuiAlias {
    alias: String,
    public_key_base64: String,
}

fn load_keystore_accounts(
    path: &PathBuf,
    aliases: &[SuiAlias],
) -> anyhow::Result<(Vec<ParsedSuiAccount>, Vec<String>)> {
    if !path.is_file() {
        return Ok((
            Vec::new(),
            vec![format!("Sui keystore not found at {}", path.display())],
        ));
    }

    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let values: Vec<JsonValue> = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse {}", path.display()))?;
    let mut accounts = Vec::new();
    let mut diagnostics = Vec::new();

    for (index, value) in values.iter().enumerate() {
        let Some(raw_key) = value.as_str() else {
            diagnostics.push(format!("Keystore entry {index} is not a string"));
            continue;
        };

        match signer_from_keystore_entry(raw_key) {
            Ok(signer) => {
                let address = signer
                    .address()
                    .map_err(|error| {
                        anyhow!("Failed to derive Sui address for keystore entry {index}: {error}")
                    })?
                    .to_string();
                let public_key_base64 = signer_public_key_base64(&signer)?;
                accounts.push(ParsedSuiAccount {
                    address: normalize_address(&address),
                    alias: aliases
                        .iter()
                        .find(|alias| alias.public_key_base64 == public_key_base64)
                        .map(|alias| alias.alias.clone()),
                    suiprivkey: Some(signer.to_suiprivkey().map_err(|error| {
                        anyhow!("Failed to encode keystore entry {index} as suiprivkey: {error}")
                    })?),
                    status: "available".to_string(),
                });
            }
            Err(error) => {
                diagnostics.push(format!("Keystore entry {index} is unsupported: {error}"))
            }
        }
    }

    Ok((accounts, diagnostics))
}

fn signer_from_keystore_entry(raw_key: &str) -> anyhow::Result<Ed25519Signer> {
    if raw_key.starts_with("suiprivkey") {
        return Ed25519Signer::from_suiprivkey(raw_key)
            .map_err(|error| anyhow!("invalid suiprivkey: {error}"));
    }

    let decoded = BASE64
        .decode(raw_key)
        .map_err(|error| anyhow!("invalid base64 key: {error}"))?;
    let (scheme, key_bytes) = decoded
        .split_first()
        .ok_or_else(|| anyhow!("empty keystore entry"))?;
    if *scheme != 0 {
        return Err(anyhow!("only Ed25519 Sui keys are supported for Memwal"));
    }
    let private_key: [u8; 32] = key_bytes
        .get(..32)
        .ok_or_else(|| anyhow!("Ed25519 key entry is missing private key bytes"))?
        .try_into()
        .map_err(|_| anyhow!("Ed25519 private key must be 32 bytes"))?;
    Ed25519Signer::from_bytes(private_key)
        .map_err(|error| anyhow!("invalid Ed25519 private key: {error}"))
}

fn signer_public_key_base64(signer: &Ed25519Signer) -> anyhow::Result<String> {
    let mut bytes = vec![0u8];
    bytes.extend_from_slice(
        &signer
            .public_key_bytes()
            .map_err(|error| anyhow!("{error}"))?,
    );
    Ok(BASE64.encode(bytes))
}

fn normalize_address(address: &str) -> String {
    let trimmed = address.trim();
    if trimmed.starts_with("0x") {
        trimmed.to_ascii_lowercase()
    } else {
        format!("0x{}", trimmed.to_ascii_lowercase())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sui_base64_keystore_entry(signer: &Ed25519Signer) -> String {
        let suiprivkey = signer.to_suiprivkey().unwrap();
        let reparsed = signer_from_keystore_entry(&suiprivkey).unwrap();
        let mut bytes = vec![0u8];
        let encoded_again = reparsed.to_suiprivkey().unwrap();
        let decoded_signer = signer_from_keystore_entry(&encoded_again).unwrap();
        let private_key = decoded_signer.to_suiprivkey().unwrap();
        let raw_signer = signer_from_keystore_entry(&private_key).unwrap();
        let raw_key = raw_signer.to_suiprivkey().unwrap();
        let decoded = memwal_core::DelegateKey::from_suiprivkey(&raw_key).unwrap();
        bytes.extend_from_slice(&hex::decode(decoded.to_hex()).unwrap());
        BASE64.encode(bytes)
    }

    fn write_sui_fixture(dir: &std::path::Path, active_address: &str, keys: Vec<String>) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            dir.join("client.yaml"),
            format!(
                "keystore:\n  File: {}\nenvs:\n  - alias: testnet\n    rpc: \"https://fullnode.testnet.sui.io:443\"\nactive_env: testnet\nactive_address: \"{}\"\n",
                dir.join("sui.keystore").display(),
                active_address
            ),
        )
        .unwrap();
        std::fs::write(
            dir.join("sui.keystore"),
            serde_json::to_string(&keys).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn missing_sui_config_is_reported_without_error() {
        let dir = std::env::temp_dir().join(format!("eidetic-missing-{}", uuid::Uuid::new_v4()));
        let state = load_sui_config(Some(dir)).expect("missing config should not hard-fail");
        assert!(state.accounts.is_empty());
        assert!(state.diagnostics[0].contains("Sui client config not found"));
    }

    #[test]
    fn parses_active_address_alias_and_keystore() {
        let dir = std::env::temp_dir().join(format!("eidetic-sui-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let signer = Ed25519Signer::generate().unwrap();
        let address = signer.address().unwrap().to_string();
        let suiprivkey = signer.to_suiprivkey().unwrap();
        let public_key_base64 = signer_public_key_base64(&signer).unwrap();
        std::fs::write(
            dir.join("client.yaml"),
            format!(
                "keystore:\n  File: {}\nenvs:\n  - alias: testnet\n    rpc: \"https://fullnode.testnet.sui.io:443\"\nactive_env: testnet\nactive_address: \"{}\"\n",
                dir.join("sui.keystore").display(),
                address
            ),
        )
        .unwrap();
        std::fs::write(
            dir.join("sui.aliases"),
            serde_json::json!([{ "alias": "primary", "public_key_base64": public_key_base64 }])
                .to_string(),
        )
        .unwrap();
        std::fs::write(
            dir.join("sui.keystore"),
            serde_json::json!([suiprivkey, "not-a-valid-key"]).to_string(),
        )
        .unwrap();

        let state = load_sui_config(Some(dir)).unwrap();
        assert_eq!(
            state.active_address.as_deref(),
            Some(normalize_address(&address).as_str())
        );
        assert_eq!(state.active_env.as_deref(), Some("testnet"));
        assert_eq!(state.accounts[0].alias.as_deref(), Some("primary"));
        assert!(state.accounts[0].suiprivkey.is_some());
        assert_eq!(state.accounts[0].status, "available");
        assert_eq!(state.diagnostics.len(), 1);
    }

    #[test]
    fn derives_expected_address_from_suiprivkey_keystore_entry() {
        let signer = Ed25519Signer::generate().unwrap();
        let expected_address = normalize_address(&signer.address().unwrap().to_string());
        let suiprivkey = signer.to_suiprivkey().unwrap();

        let derived = signer_from_keystore_entry(&suiprivkey).unwrap();
        let derived_address = normalize_address(&derived.address().unwrap().to_string());

        assert_eq!(derived_address, expected_address);
        assert_eq!(derived.to_suiprivkey().unwrap(), suiprivkey);
    }

    #[test]
    fn derives_expected_address_from_base64_sui_keystore_entry() {
        let signer = Ed25519Signer::generate().unwrap();
        let expected_address = normalize_address(&signer.address().unwrap().to_string());
        let keystore_entry = sui_base64_keystore_entry(&signer);

        let derived = signer_from_keystore_entry(&keystore_entry).unwrap();
        let derived_address = normalize_address(&derived.address().unwrap().to_string());

        assert_eq!(derived_address, expected_address);
    }

    #[test]
    fn load_sui_config_derives_addresses_for_mixed_keystore_entries() {
        let dir = std::env::temp_dir().join(format!("eidetic-mixed-sui-{}", uuid::Uuid::new_v4()));
        let first = Ed25519Signer::generate().unwrap();
        let second = Ed25519Signer::generate().unwrap();
        let first_address = normalize_address(&first.address().unwrap().to_string());
        let second_address = normalize_address(&second.address().unwrap().to_string());
        write_sui_fixture(
            &dir,
            &first_address,
            vec![
                first.to_suiprivkey().unwrap(),
                sui_base64_keystore_entry(&second),
            ],
        );

        let state = load_sui_config(Some(dir)).unwrap();
        let addresses = state
            .accounts
            .iter()
            .map(|account| account.address.clone())
            .collect::<Vec<_>>();

        assert!(addresses.contains(&first_address));
        assert!(addresses.contains(&second_address));
        assert!(state.diagnostics.is_empty());
    }

    #[tokio::test]
    async fn auth_manager_selects_active_address_from_sui_config() {
        let dir = std::env::temp_dir().join(format!("eidetic-active-sui-{}", uuid::Uuid::new_v4()));
        let inactive = Ed25519Signer::generate().unwrap();
        let active = Ed25519Signer::generate().unwrap();
        let active_address = normalize_address(&active.address().unwrap().to_string());
        write_sui_fixture(
            &dir,
            &active_address,
            vec![
                inactive.to_suiprivkey().unwrap(),
                active.to_suiprivkey().unwrap(),
            ],
        );

        let manager = AuthManager::new(MemwalAuthConfig {
            sui_config_dir: Some(dir),
            ..MemwalAuthConfig::default()
        })
        .await
        .unwrap();
        let snapshot = manager.config_snapshot().await.unwrap();

        assert_eq!(
            snapshot.selected_address.as_deref(),
            Some(active_address.as_str())
        );
        assert!(!snapshot.provisioned);
    }

    #[test]
    #[ignore = "depends on the developer machine's ~/.sui/sui_config"]
    fn loads_real_sui_config_from_home() {
        let dir = default_sui_config_dir();
        let state = load_sui_config(Some(dir.clone())).unwrap_or_else(|error| {
            panic!(
                "failed to load real Sui config at {}: {error}",
                dir.display()
            )
        });

        assert!(
            !state.diagnostics.iter().any(|diagnostic| diagnostic
                .contains("Sui client config not found")
                || diagnostic.contains("No usable Sui accounts found")),
            "real Sui config diagnostics: {:?}",
            state.diagnostics
        );
        assert!(
            !state.accounts.is_empty(),
            "real Sui config should expose at least one account"
        );
        assert!(
            state
                .accounts
                .iter()
                .any(|account| account.suiprivkey.is_some()),
            "real Sui config should expose at least one usable Ed25519 private key"
        );
        if let Some(active_address) = state.active_address.as_deref() {
            assert!(
                state
                    .accounts
                    .iter()
                    .any(|account| account.address.eq_ignore_ascii_case(active_address)),
                "active address should match a derived account"
            );
        }
    }
}
