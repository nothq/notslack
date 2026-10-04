use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

use anyhow::{anyhow, Context, Result};

use crate::AUTH_FILE_NAME;

const NOTSLACK_AUTH_PATH_ENV: &str = "NOTSLACK_AUTH_PATH";

pub(crate) fn credential_file_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

pub(crate) fn default_auth_path() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os(NOTSLACK_AUTH_PATH_ENV) {
        let path = PathBuf::from(path);
        if !path.is_absolute() {
            return Err(anyhow!("{NOTSLACK_AUTH_PATH_ENV} must be an absolute path"));
        }
        return Ok(path);
    }
    Ok(home_dir()?.join(".notslack").join(AUTH_FILE_NAME))
}

fn home_dir() -> Result<PathBuf> {
    if let Some(home) = std::env::var_os("HOME") {
        return Ok(PathBuf::from(home));
    }
    #[cfg(target_os = "windows")]
    if let Some(home) = std::env::var_os("USERPROFILE") {
        return Ok(PathBuf::from(home));
    }
    Err(anyhow!("HOME was not set for notslack credentials"))
}

pub(crate) fn validate_cache_key(cache_key: &str) -> Result<()> {
    if cache_key.trim().is_empty() {
        return Err(anyhow!("cache key must not be empty"));
    }
    Ok(())
}

pub(crate) fn non_empty_value<'a>(value: &'a str, label: &str) -> Result<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        return Err(anyhow!("{label} must not be empty"));
    }
    Ok(value)
}

pub(crate) fn credentials_temp_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(AUTH_FILE_NAME);
    path.with_file_name(format!(".{file_name}.tmp-{}", std::process::id()))
}

pub(crate) fn create_credentials_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path).with_context(|| {
        format!(
            "failed to create notslack credentials directory {}",
            path.display()
        )
    })?;
    set_credentials_dir_permissions(path)?;
    Ok(())
}

pub(crate) fn write_credentials_file(path: &Path, contents: &[u8]) -> Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    configure_private_file_options(&mut options);
    let mut file = options
        .open(path)
        .with_context(|| format!("failed to write notslack credentials {}", path.display()))?;
    file.write_all(contents)
        .with_context(|| format!("failed to write notslack credentials {}", path.display()))?;
    file.flush()
        .with_context(|| format!("failed to flush notslack credentials {}", path.display()))?;
    Ok(())
}

#[cfg(unix)]
fn configure_private_file_options(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;
    options.mode(0o600);
}

#[cfg(not(unix))]
fn configure_private_file_options(_options: &mut OpenOptions) {}

#[cfg(unix)]
fn set_credentials_dir_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).with_context(|| {
        format!(
            "failed to set notslack credentials directory permissions {}",
            path.display()
        )
    })
}

#[cfg(not(unix))]
fn set_credentials_dir_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
pub(crate) fn set_credentials_file_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).with_context(|| {
        format!(
            "failed to set notslack credentials file permissions {}",
            path.display()
        )
    })
}

#[cfg(not(unix))]
pub(crate) fn set_credentials_file_permissions(_path: &Path) -> Result<()> {
    Ok(())
}
