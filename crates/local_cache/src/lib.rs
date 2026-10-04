mod crypto;
mod keychain;
mod keys;
mod paths;
mod runtime_state;

#[cfg(any(test, feature = "test-support"))]
pub mod test_support;

pub use crypto::{
    open_bytes_v1, read_encrypted_json, read_encrypted_json_strict_bounded, seal_bytes_v1,
    write_encrypted_json, write_encrypted_json_durable_bounded, DurableEncryptedWriteError,
};
pub use keychain::load_or_create_keychain_key;
pub use keys::{
    cache_key_hash, decode_cache_key, encode_cache_key, generate_cache_key,
    load_or_create_cache_key,
};
pub use paths::{account_cache_dir, default_cache_root_dir};
pub use runtime_state::{load_runtime_state_json, mutate_runtime_state_json};

#[cfg(test)]
mod tests;
