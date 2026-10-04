use std::path::{Path, PathBuf};

use crate::cache_key_hash;

pub fn default_cache_root_dir(
    env_key: &str,
    app_subdir: &str,
    description: &str,
) -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os(env_key) {
        return Ok(PathBuf::from(path));
    }

    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| format!("HOME was not set for notslack {description} cache"))?;
    #[cfg(target_os = "macos")]
    {
        Ok(home.join("Library/Caches/dev.nothq.notslack").join(app_subdir))
    }
    #[cfg(not(target_os = "macos"))]
    {
        if let Some(path) = std::env::var_os("XDG_CACHE_HOME") {
            return Ok(PathBuf::from(path).join("dev.nothq.notslack").join(app_subdir));
        }
        Ok(home.join(".cache/dev.nothq.notslack").join(app_subdir))
    }
}

pub fn account_cache_dir(cache_root: &Path, cache_key: &str) -> PathBuf {
    cache_root.join(cache_key_hash(cache_key))
}
