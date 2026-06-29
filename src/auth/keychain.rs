use anyhow::{Context, Result};
use keyring::Entry;

const KEYRING_SERVICE: &str = "eidetic-core";
const KEYRING_PRIVATE_KEY: &str = "private-key";

pub struct KeychainManager;

impl KeychainManager {
    pub fn store_private_key(private_key: &str) -> Result<()> {
        let entry = Entry::new(KEYRING_SERVICE, KEYRING_PRIVATE_KEY)
            .context("Failed to create keyring entry for private key")?;
        entry
            .set_password(private_key)
            .context("Failed to store private key in system keychain")?;
        Ok(())
    }

    pub fn load_private_key() -> Result<String> {
        let entry = Entry::new(KEYRING_SERVICE, KEYRING_PRIVATE_KEY)
            .context("Failed to create keyring entry for private key")?;
        entry
            .get_password()
            .context("Private key not found in keychain")
    }

    pub fn is_configured() -> bool {
        Self::load_private_key().is_ok()
    }

}
