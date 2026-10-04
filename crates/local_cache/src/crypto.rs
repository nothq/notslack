mod byte_envelope;
mod durable_json;

pub use byte_envelope::{open_bytes_v1, seal_bytes_v1};
pub use durable_json::{
    read_encrypted_json_strict_bounded, write_encrypted_json_durable_bounded,
    DurableEncryptedWriteError,
};

use std::{fs, path::Path};

use aes_gcm_siv::{
    aead::{Aead, KeyInit},
    Aes256GcmSiv, Nonce,
};
use rand::{rngs::OsRng, RngCore};
use serde::{de::DeserializeOwned, Serialize};

const CACHE_MAGIC: &[u8] = b"OTLC1";
const CACHE_NONCE_BYTES: usize = 12;

pub fn read_encrypted_json<T: DeserializeOwned>(
    path: &Path,
    key: &[u8; 32],
) -> Result<Option<T>, String> {
    let ciphertext = match fs::read(path) {
        Ok(ciphertext) => ciphertext,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "failed to read notslack cache {}: {error}",
                path.display()
            ));
        }
    };
    let plaintext = match decrypt_cache_bytes(key, &ciphertext) {
        Ok(plaintext) => plaintext,
        Err(_) => return Ok(None),
    };
    serde_json::from_slice(&plaintext).map(Some).or(Ok(None))
}

pub fn write_encrypted_json<T: Serialize>(
    path: &Path,
    key: &[u8; 32],
    value: &T,
) -> Result<(), String> {
    let plaintext = serde_json::to_vec(value)
        .map_err(|error| format!("failed to encode notslack cache {}: {error}", path.display()))?;
    let ciphertext = encrypt_cache_bytes(key, &plaintext)?;
    let parent = path.parent().ok_or_else(|| {
        format!(
            "cache path {} was missing a parent directory",
            path.display()
        )
    })?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "failed to create notslack cache directory {}: {error}",
            parent.display()
        )
    })?;
    let temp_path = parent.join(format!(
        ".{}.tmp-{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("cache"),
        random_temp_suffix()
    ));
    fs::write(&temp_path, ciphertext).map_err(|error| {
        format!(
            "failed to write notslack cache temp file {}: {error}",
            temp_path.display()
        )
    })?;
    fs::rename(&temp_path, path).map_err(|error| {
        let _ = fs::remove_file(&temp_path);
        format!("failed to finalize notslack cache {}: {error}", path.display())
    })
}

fn encrypt_cache_bytes(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>, String> {
    let cipher = Aes256GcmSiv::new_from_slice(key)
        .map_err(|error| format!("failed to initialize cache cipher: {error}"))?;
    let mut nonce = [0_u8; CACHE_NONCE_BYTES];
    OsRng.fill_bytes(&mut nonce);
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), plaintext)
        .map_err(|error| format!("failed to encrypt notslack cache: {error}"))?;
    let mut encoded = Vec::with_capacity(CACHE_MAGIC.len() + nonce.len() + ciphertext.len());
    encoded.extend_from_slice(CACHE_MAGIC);
    encoded.extend_from_slice(&nonce);
    encoded.extend_from_slice(&ciphertext);
    Ok(encoded)
}

fn decrypt_cache_bytes(key: &[u8; 32], ciphertext: &[u8]) -> Result<Vec<u8>, String> {
    if ciphertext.len() < CACHE_MAGIC.len() + CACHE_NONCE_BYTES {
        return Err("cache ciphertext was too short".to_string());
    }
    if &ciphertext[..CACHE_MAGIC.len()] != CACHE_MAGIC {
        return Err("cache magic header mismatch".to_string());
    }
    let nonce_start = CACHE_MAGIC.len();
    let nonce_end = nonce_start + CACHE_NONCE_BYTES;
    let cipher = Aes256GcmSiv::new_from_slice(key)
        .map_err(|error| format!("failed to initialize cache cipher: {error}"))?;
    cipher
        .decrypt(
            Nonce::from_slice(&ciphertext[nonce_start..nonce_end]),
            &ciphertext[nonce_end..],
        )
        .map_err(|error| format!("failed to decrypt notslack cache: {error}"))
}

fn random_temp_suffix() -> u64 {
    OsRng.next_u64()
}
