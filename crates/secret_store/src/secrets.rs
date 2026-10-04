use anyhow::Result;

use crate::{
    filesystem::{non_empty_value, validate_cache_key},
    SecretStore,
};

impl SecretStore {
    pub fn read_secret(&self, cache_key: &str) -> Result<Option<String>> {
        validate_cache_key(cache_key)?;
        self.load_credentials()
            .map(|credentials| credentials.secrets.get(cache_key).cloned())
    }

    pub fn upsert_secret(&self, cache_key: &str, secret: &str) -> Result<()> {
        validate_cache_key(cache_key)?;
        let secret = non_empty_value(secret, "secret value")?;
        self.update_credentials(|credentials| {
            credentials
                .secrets
                .insert(cache_key.to_string(), secret.to_string());
        })
    }

    pub fn remove_secret(&self, cache_key: &str) -> Result<()> {
        validate_cache_key(cache_key)?;
        self.update_credentials(|credentials| {
            credentials.secrets.remove(cache_key);
        })
    }
}
