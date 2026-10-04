use std::{
    error::Error,
    fmt,
    fs::{self, File},
    io::{Read, Write},
    path::Path,
};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt as _;

use serde::{de::DeserializeOwned, Serialize};

use super::{decrypt_cache_bytes, encrypt_cache_bytes};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DurableEncryptedWriteCommitState {
    NotCommitted,
    Ambiguous,
}

#[derive(Debug)]
pub struct DurableEncryptedWriteError {
    commit_state: DurableEncryptedWriteCommitState,
    message: String,
}

impl DurableEncryptedWriteError {
    fn not_committed(message: String) -> Self {
        Self {
            commit_state: DurableEncryptedWriteCommitState::NotCommitted,
            message,
        }
    }

    fn commit_state_ambiguous(message: String) -> Self {
        Self {
            commit_state: DurableEncryptedWriteCommitState::Ambiguous,
            message,
        }
    }

    pub fn commit_state_is_ambiguous(&self) -> bool {
        self.commit_state == DurableEncryptedWriteCommitState::Ambiguous
    }
}

impl fmt::Display for DurableEncryptedWriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)?;
        if self.commit_state_is_ambiguous() {
            formatter.write_str("; durable commit state is ambiguous")?;
        }
        Ok(())
    }
}

impl Error for DurableEncryptedWriteError {}

pub fn read_encrypted_json_strict_bounded<T: DeserializeOwned>(
    path: &Path,
    key: &[u8; 32],
    max_bytes: usize,
) -> Result<Option<T>, String> {
    let Some(mut file) = open_optional(path)? else {
        return Ok(None);
    };
    let metadata = file.metadata().map_err(|error| {
        format!(
            "failed to inspect encrypted notslack cache {}: {error}",
            path.display()
        )
    })?;
    let maximum = u64::try_from(max_bytes)
        .map_err(|_| "encrypted notslack cache byte limit exceeds u64".to_string())?;
    if metadata.len() > maximum {
        return Err(size_error(path, max_bytes));
    }
    let mut ciphertext = Vec::with_capacity(metadata.len().try_into().unwrap_or(max_bytes));
    Read::by_ref(&mut file)
        .take(
            maximum
                .checked_add(1)
                .ok_or("encrypted cache limit overflowed")?,
        )
        .read_to_end(&mut ciphertext)
        .map_err(|error| {
            format!(
                "failed to read encrypted notslack cache {}: {error}",
                path.display()
            )
        })?;
    if ciphertext.len() > max_bytes {
        return Err(size_error(path, max_bytes));
    }
    let plaintext = decrypt_cache_bytes(key, &ciphertext).map_err(|error| {
        format!(
            "failed to decrypt encrypted notslack cache {}: {error}",
            path.display()
        )
    })?;
    serde_json::from_slice(&plaintext)
        .map(Some)
        .map_err(|error| {
            format!(
                "failed to decode encrypted notslack cache {}: {error}",
                path.display()
            )
        })
}

pub fn write_encrypted_json_durable_bounded<T: Serialize>(
    path: &Path,
    key: &[u8; 32],
    value: &T,
    max_bytes: usize,
) -> Result<(), DurableEncryptedWriteError> {
    let ciphertext = encoded_value(path, key, value, max_bytes)?;
    let parent = path.parent().ok_or_else(|| {
        DurableEncryptedWriteError::not_committed(format!(
            "encrypted notslack cache path {} was missing a parent directory",
            path.display()
        ))
    })?;
    fs::create_dir_all(parent).map_err(|error| {
        DurableEncryptedWriteError::not_committed(format!(
            "failed to create encrypted notslack cache directory {}: {error}",
            parent.display()
        ))
    })?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|error| {
        DurableEncryptedWriteError::not_committed(format!(
            "failed to create encrypted notslack cache temp file in {}: {error}",
            parent.display()
        ))
    })?;
    secure_and_write(parent, &mut temporary, &ciphertext)?;
    temporary.persist(path).map_err(|error| {
        DurableEncryptedWriteError::not_committed(format!(
            "failed to atomically replace encrypted notslack cache {}: {}",
            path.display(),
            error.error
        ))
    })?;
    sync_parent(parent)?;
    Ok(())
}

fn open_optional(path: &Path) -> Result<Option<File>, String> {
    match File::open(path) {
        Ok(file) => Ok(Some(file)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!(
            "failed to open encrypted notslack cache {}: {error}",
            path.display()
        )),
    }
}

fn encoded_value<T: Serialize>(
    path: &Path,
    key: &[u8; 32],
    value: &T,
    max_bytes: usize,
) -> Result<Vec<u8>, DurableEncryptedWriteError> {
    let plaintext = serde_json::to_vec(value).map_err(|_| {
        DurableEncryptedWriteError::not_committed(format!(
            "failed to encode encrypted notslack cache {}",
            path.display()
        ))
    })?;
    if plaintext.len() > max_bytes {
        return Err(DurableEncryptedWriteError::not_committed(size_error(
            path, max_bytes,
        )));
    }
    let ciphertext =
        encrypt_cache_bytes(key, &plaintext).map_err(DurableEncryptedWriteError::not_committed)?;
    if ciphertext.len() > max_bytes {
        return Err(DurableEncryptedWriteError::not_committed(size_error(
            path, max_bytes,
        )));
    }
    Ok(ciphertext)
}

fn secure_and_write(
    parent: &Path,
    temporary: &mut tempfile::NamedTempFile,
    ciphertext: &[u8],
) -> Result<(), DurableEncryptedWriteError> {
    #[cfg(unix)]
    temporary
        .as_file_mut()
        .set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(|error| not_committed(parent, "secure", error))?;
    temporary
        .write_all(ciphertext)
        .map_err(|error| not_committed(parent, "write", error))?;
    temporary
        .as_file_mut()
        .sync_all()
        .map_err(|error| not_committed(parent, "sync", error))
}

fn not_committed(parent: &Path, action: &str, error: std::io::Error) -> DurableEncryptedWriteError {
    DurableEncryptedWriteError::not_committed(format!(
        "failed to {action} encrypted notslack cache temp file in {}: {error}",
        parent.display()
    ))
}

#[cfg(unix)]
fn sync_parent(parent: &Path) -> Result<(), DurableEncryptedWriteError> {
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| {
            DurableEncryptedWriteError::commit_state_ambiguous(format!(
                "failed to sync encrypted notslack cache directory {}: {error}",
                parent.display()
            ))
        })
}

#[cfg(not(unix))]
fn sync_parent(_parent: &Path) -> Result<(), DurableEncryptedWriteError> {
    Ok(())
}

fn size_error(path: &Path, max_bytes: usize) -> String {
    format!(
        "encrypted notslack cache {} exceeds its {max_bytes}-byte limit",
        path.display()
    )
}
