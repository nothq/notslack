use std::fs;

use serde::{Deserialize, Serialize};
use tempfile::TempDir;

use crate::{
    generate_cache_key, open_bytes_v1, read_encrypted_json, seal_bytes_v1, write_encrypted_json,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct TestCache {
    value: String,
}

#[test]
fn encrypted_cache_round_trips() {
    let tempdir = TempDir::new().expect("tempdir");
    let key = generate_cache_key();
    let path = tempdir.path().join("cache.bin");
    let cache = TestCache {
        value: "secret payload".to_string(),
    };

    write_encrypted_json(&path, &key, &cache).expect("write cache");
    let loaded = read_encrypted_json::<TestCache>(&path, &key)
        .expect("read cache")
        .expect("cache should exist");

    assert_eq!(loaded, cache);
    let ciphertext = fs::read(&path).expect("read ciphertext");
    assert!(!String::from_utf8_lossy(&ciphertext).contains("secret payload"));
}

#[test]
fn corrupted_cache_is_ignored() {
    let tempdir = TempDir::new().expect("tempdir");
    let key = generate_cache_key();
    let path = tempdir.path().join("cache.bin");
    fs::write(&path, b"not-an-encrypted-cache").expect("write corrupt cache");

    let loaded = read_encrypted_json::<TestCache>(&path, &key).expect("read corrupt cache");

    assert!(loaded.is_none());
}

#[test]
fn associated_data_byte_envelope_round_trips() {
    let key = generate_cache_key();
    let body = b"secret artifact bytes";
    let aad = b"browser|broker|scope-a|artifact|artifact:a1";

    let sealed = seal_bytes_v1(&key, aad, body, 64, 128).expect("seal bytes");
    let opened = open_bytes_v1(&key, aad, &sealed, 64, 128).expect("open bytes");

    assert_eq!(opened.as_slice(), body);
    assert!(!sealed.windows(body.len()).any(|window| window == body));
}

#[test]
fn associated_data_byte_envelope_rejects_wrong_key_aad_and_tampering() {
    let key = generate_cache_key();
    let aad = b"browser|broker|scope-a|artifact|artifact:a1";
    let sealed = seal_bytes_v1(&key, aad, b"body", 16, 128).expect("seal bytes");

    assert!(open_bytes_v1(&generate_cache_key(), aad, &sealed, 16, 128).is_err());
    assert!(open_bytes_v1(&key, b"different record", &sealed, 16, 128).is_err());
    let mut tampered = sealed;
    let last = tampered.last_mut().expect("ciphertext byte");
    *last ^= 1;
    assert!(open_bytes_v1(&key, aad, &tampered, 16, 128).is_err());
}

#[test]
fn associated_data_byte_envelope_enforces_all_bounds() {
    let key = generate_cache_key();

    assert!(seal_bytes_v1(&key, b"", b"body", 16, 128).is_err());
    assert!(seal_bytes_v1(&key, b"record", b"too long", 4, 128).is_err());
    assert!(seal_bytes_v1(&key, b"record", b"body", 16, 8).is_err());

    let sealed = seal_bytes_v1(&key, b"record", b"body", 16, 128).expect("seal bytes");
    assert!(open_bytes_v1(&key, b"record", &sealed, 3, 128).is_err());
    assert!(open_bytes_v1(&key, b"record", &sealed, 16, sealed.len() - 1).is_err());
}
