use base64::Engine as _;
use rand::{rngs::OsRng, RngCore};
use sha2::{Digest, Sha256};

pub fn cache_key_hash(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        encoded.push(hex_char(byte >> 4));
        encoded.push(hex_char(byte & 0x0f));
    }
    encoded
}

pub fn generate_cache_key() -> [u8; 32] {
    let mut key = [0_u8; 32];
    OsRng.fill_bytes(&mut key);
    key
}

pub fn encode_cache_key(key: &[u8; 32]) -> String {
    base64::engine::general_purpose::STANDARD.encode(key)
}

pub fn decode_cache_key(encoded: &str) -> Result<[u8; 32], String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|error| format!("invalid cache key encoding: {error}"))?;
    if bytes.len() != 32 {
        return Err(format!(
            "invalid cache key length {}; expected 32 bytes",
            bytes.len()
        ));
    }
    let mut key = [0_u8; 32];
    key.copy_from_slice(&bytes);
    Ok(key)
}

pub fn load_or_create_cache_key(cache_key: &str, description: &str) -> Result<[u8; 32], String> {
    if let Some(encoded_key) = load_cache_key(cache_key, description)? {
        return decode_cache_key(&encoded_key);
    }

    let key = generate_cache_key();
    store_cache_key(cache_key, &encode_cache_key(&key), description)?;
    Ok(key)
}

#[cfg(any(test, feature = "test-support"))]
fn load_cache_key(cache_key: &str, _description: &str) -> Result<Option<String>, String> {
    let override_store = crate::test_support::cache_key_test_override()
        .lock()
        .expect("cache key override lock");
    if let Some(override_store) = override_store.as_ref() {
        return (override_store.read)(cache_key.to_string());
    }
    secret_store::SecretStore::notslack()
        .and_then(|store| store.read_cache_key(cache_key))
        .map_err(|error| format!("failed to read notslack cache key: {error:#}"))
}

#[cfg(not(any(test, feature = "test-support")))]
fn load_cache_key(cache_key: &str, description: &str) -> Result<Option<String>, String> {
    secret_store::SecretStore::notslack()
        .and_then(|store| store.read_cache_key(cache_key))
        .map_err(|error| format!("failed to read notslack {description} cache key: {error:#}"))
}

#[cfg(any(test, feature = "test-support"))]
fn store_cache_key(cache_key: &str, key: &str, _description: &str) -> Result<(), String> {
    let override_store = crate::test_support::cache_key_test_override()
        .lock()
        .expect("cache key override lock");
    if let Some(override_store) = override_store.as_ref() {
        return (override_store.write)(cache_key.to_string(), key.to_string());
    }
    secret_store::SecretStore::notslack()
        .and_then(|store| store.upsert_cache_key(cache_key, key))
        .map_err(|error| format!("failed to store notslack cache key: {error:#}"))
}

#[cfg(not(any(test, feature = "test-support")))]
fn store_cache_key(cache_key: &str, key: &str, description: &str) -> Result<(), String> {
    secret_store::SecretStore::notslack()
        .and_then(|store| store.upsert_cache_key(cache_key, key))
        .map_err(|error| format!("failed to store notslack {description} cache key: {error:#}"))
}

fn hex_char(value: u8) -> char {
    match value {
        0..=9 => char::from(b'0' + value),
        10..=15 => char::from(b'a' + (value - 10)),
        _ => unreachable!("hex nibble must be in range 0..=15"),
    }
}
