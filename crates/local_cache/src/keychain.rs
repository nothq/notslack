#[cfg(target_os = "macos")]
use rand::{rngs::OsRng, RngCore};

const KEY_BYTES: usize = 32;

pub fn load_or_create_keychain_key(
    service: &str,
    account: &str,
    create_if_missing: bool,
) -> Result<[u8; KEY_BYTES], String> {
    #[cfg(target_os = "macos")]
    {
        use security_framework::passwords::{get_generic_password, set_generic_password};
        use security_framework_sys::base::errSecItemNotFound;

        match get_generic_password(service, account) {
            Ok(key) => decode_keychain_key(service, account, key),
            Err(error) if error.code() == errSecItemNotFound => {
                if !create_if_missing {
                    return Err(format!(
                        "macOS Keychain entry {service}/{account} is missing"
                    ));
                }
                let mut key = [0_u8; KEY_BYTES];
                OsRng.fill_bytes(&mut key);
                set_generic_password(service, account, &key).map_err(|error| {
                    format!("failed to save macOS Keychain entry {service}/{account}: {error}")
                })?;
                Ok(key)
            }
            Err(error) => Err(format!(
                "failed to load macOS Keychain entry {service}/{account}: {error}"
            )),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (service, account, create_if_missing);
        Err("encrypted notslack ledger keys require macOS Keychain".to_string())
    }
}

#[cfg(target_os = "macos")]
fn decode_keychain_key(
    service: &str,
    account: &str,
    key: Vec<u8>,
) -> Result<[u8; KEY_BYTES], String> {
    key.try_into().map_err(|key: Vec<u8>| {
        format!(
            "macOS Keychain entry {service}/{account} contains {} bytes; expected {KEY_BYTES}",
            key.len()
        )
    })
}
