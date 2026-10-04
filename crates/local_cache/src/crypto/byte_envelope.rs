use aes_gcm_siv::{
    aead::{Aead, KeyInit, Payload},
    Aes256GcmSiv, Nonce,
};
use rand::{rngs::OsRng, RngCore};
use zeroize::Zeroizing;

const BYTE_ENVELOPE_MAGIC: &[u8] = b"OTAE1";
const BYTE_ENVELOPE_NONCE_BYTES: usize = 12;
const MAX_ASSOCIATED_DATA_BYTES: usize = 4 * 1024;

/// Seals bounded bytes in the versioned notslack AEAD envelope.
///
/// `associated_data` is part of the authentication boundary and must identify
/// the owning store, namespace, record kind, and immutable record identity.
pub fn seal_bytes_v1(
    key: &[u8; 32],
    associated_data: &[u8],
    plaintext: &[u8],
    max_plaintext_bytes: usize,
    max_ciphertext_bytes: usize,
) -> Result<Vec<u8>, String> {
    validate_input(
        associated_data,
        plaintext.len(),
        max_plaintext_bytes,
        "plaintext",
    )?;
    let encoded_len = encoded_length(plaintext.len())?;
    if encoded_len > max_ciphertext_bytes {
        return Err("encrypted byte envelope exceeds its ciphertext limit".to_string());
    }
    let cipher = cipher(key)?;
    let mut nonce = [0_u8; BYTE_ENVELOPE_NONCE_BYTES];
    OsRng
        .try_fill_bytes(&mut nonce)
        .map_err(|error| format!("failed to generate byte-envelope nonce: {error}"))?;
    let ciphertext = cipher
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad: associated_data,
            },
        )
        .map_err(|error| format!("failed to encrypt byte envelope: {error}"))?;
    let mut encoded = Vec::with_capacity(encoded_len);
    encoded.extend_from_slice(BYTE_ENVELOPE_MAGIC);
    encoded.extend_from_slice(&nonce);
    encoded.extend_from_slice(&ciphertext);
    Ok(encoded)
}

/// Opens bytes produced by [`seal_bytes_v1`] after enforcing both bounds.
pub fn open_bytes_v1(
    key: &[u8; 32],
    associated_data: &[u8],
    ciphertext: &[u8],
    max_plaintext_bytes: usize,
    max_ciphertext_bytes: usize,
) -> Result<Zeroizing<Vec<u8>>, String> {
    validate_input(
        associated_data,
        ciphertext.len(),
        max_ciphertext_bytes,
        "ciphertext",
    )?;
    let header_bytes = BYTE_ENVELOPE_MAGIC.len() + BYTE_ENVELOPE_NONCE_BYTES;
    if ciphertext.len() < header_bytes + 16 {
        return Err("encrypted byte envelope was too short".to_string());
    }
    if &ciphertext[..BYTE_ENVELOPE_MAGIC.len()] != BYTE_ENVELOPE_MAGIC {
        return Err("encrypted byte envelope magic mismatch".to_string());
    }
    let nonce_start = BYTE_ENVELOPE_MAGIC.len();
    let nonce_end = nonce_start + BYTE_ENVELOPE_NONCE_BYTES;
    if ciphertext.len() - header_bytes - 16 > max_plaintext_bytes {
        return Err("encrypted byte envelope exceeds its plaintext limit".to_string());
    }
    let plaintext = Zeroizing::new(
        cipher(key)?
            .decrypt(
                Nonce::from_slice(&ciphertext[nonce_start..nonce_end]),
                Payload {
                    msg: &ciphertext[nonce_end..],
                    aad: associated_data,
                },
            )
            .map_err(|error| format!("failed to decrypt byte envelope: {error}"))?,
    );
    if plaintext.len() > max_plaintext_bytes {
        return Err("decrypted byte envelope exceeds its plaintext limit".to_string());
    }
    Ok(plaintext)
}

fn encoded_length(plaintext_bytes: usize) -> Result<usize, String> {
    BYTE_ENVELOPE_MAGIC
        .len()
        .checked_add(BYTE_ENVELOPE_NONCE_BYTES)
        .and_then(|length| length.checked_add(plaintext_bytes))
        .and_then(|length| length.checked_add(16))
        .ok_or_else(|| "encrypted byte envelope length overflowed".to_string())
}

fn cipher(key: &[u8; 32]) -> Result<Aes256GcmSiv, String> {
    Aes256GcmSiv::new_from_slice(key)
        .map_err(|error| format!("failed to initialize byte-envelope cipher: {error}"))
}

fn validate_input(
    associated_data: &[u8],
    byte_count: usize,
    maximum: usize,
    kind: &str,
) -> Result<(), String> {
    if associated_data.is_empty() || associated_data.len() > MAX_ASSOCIATED_DATA_BYTES {
        return Err("byte-envelope associated data must be bounded and nonempty".to_string());
    }
    if byte_count > maximum {
        return Err(format!("byte-envelope {kind} exceeds its limit"));
    }
    Ok(())
}
