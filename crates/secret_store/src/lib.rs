use std::path::PathBuf;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

const AUTH_FILE_NAME: &str = "auth.json";
const CREDENTIAL_FILE_VERSION: u32 = 1;

mod credential_file;
mod filesystem;
mod secrets;

use credential_file::CredentialFile;
use filesystem::*;

pub struct SecretStore {
    auth_path: PathBuf,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct StoredAccessToken {
    pub access_token: String,
    pub expires_at_epoch_seconds: u64,
}

impl SecretStore {
    pub fn notslack() -> Result<Self> {
        Ok(Self {
            auth_path: default_auth_path()?,
        })
    }

    pub fn read_refresh_token(
        &self,
        cache_key: &str,
        _access_reason: &str,
    ) -> Result<Option<String>> {
        validate_cache_key(cache_key)?;
        self.load_credentials()
            .map(|credentials| credentials.refresh_tokens.get(cache_key).cloned())
    }

    pub fn upsert_refresh_token(&self, cache_key: &str, refresh_token: &str) -> Result<()> {
        validate_cache_key(cache_key)?;
        let refresh_token = non_empty_value(refresh_token, "refresh token")?;
        self.update_credentials(|credentials| {
            credentials
                .refresh_tokens
                .insert(cache_key.to_string(), refresh_token.to_string());
        })
    }

    pub fn has_refresh_token(&self, cache_key: &str) -> Result<bool> {
        validate_cache_key(cache_key)?;
        self.load_credentials()
            .map(|credentials| credentials.refresh_tokens.contains_key(cache_key))
    }

    pub fn read_access_token(&self, cache_key: &str) -> Result<Option<StoredAccessToken>> {
        validate_cache_key(cache_key)?;
        self.load_credentials()
            .map(|credentials| credentials.access_tokens.get(cache_key).cloned())
    }

    pub fn upsert_access_token(
        &self,
        cache_key: &str,
        access_token: &str,
        expires_at_epoch_seconds: u64,
    ) -> Result<()> {
        validate_cache_key(cache_key)?;
        let access_token = non_empty_value(access_token, "access token")?;
        self.update_credentials(|credentials| {
            credentials.access_tokens.insert(
                cache_key.to_string(),
                StoredAccessToken {
                    access_token: access_token.to_string(),
                    expires_at_epoch_seconds,
                },
            );
        })
    }

    pub fn remove_access_token(&self, cache_key: &str) -> Result<()> {
        validate_cache_key(cache_key)?;
        self.update_credentials(|credentials| {
            credentials.access_tokens.remove(cache_key);
        })
    }

    pub fn read_cache_key(&self, cache_key: &str) -> Result<Option<String>> {
        validate_cache_key(cache_key)?;
        self.load_credentials()
            .map(|credentials| credentials.cache_keys.get(cache_key).cloned())
    }

    pub fn upsert_cache_key(&self, cache_key: &str, key: &str) -> Result<()> {
        validate_cache_key(cache_key)?;
        let key = non_empty_value(key, "cache key")?;
        self.update_credentials(|credentials| {
            credentials
                .cache_keys
                .insert(cache_key.to_string(), key.to_string());
        })
    }

    fn load_credentials(&self) -> Result<CredentialFile> {
        let _guard = credential_file_lock()
            .lock()
            .map_err(|error| anyhow!("notslack credential file lock poisoned: {error}"))?;
        CredentialFile::load(&self.auth_path)
    }

    fn update_credentials(&self, update: impl FnOnce(&mut CredentialFile)) -> Result<()> {
        let _guard = credential_file_lock()
            .lock()
            .map_err(|error| anyhow!("notslack credential file lock poisoned: {error}"))?;
        let mut credentials = CredentialFile::load(&self.auth_path)?;
        update(&mut credentials);
        credentials.save(&self.auth_path)
    }

    #[cfg(test)]
    fn for_test(auth_path: impl Into<PathBuf>) -> Self {
        Self {
            auth_path: auth_path.into(),
        }
    }
}

#[cfg(test)]
mod tests;
