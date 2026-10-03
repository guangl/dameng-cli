//! AES-GCM byte format shared by stored secrets: 12-byte nonce, then ciphertext.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use rand::{TryRng, rngs::SysRng};

/// Encrypt with a fresh nonce, preserving the existing on-disk byte format.
pub fn seal(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>, aes_gcm::Error> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| aes_gcm::Error)?;
    let mut nonce = [0_u8; 12];
    SysRng
        .try_fill_bytes(&mut nonce)
        .map_err(|_| aes_gcm::Error)?;
    let ciphertext = cipher.encrypt(&Nonce::from(nonce), plaintext)?;
    let mut bytes = nonce.to_vec();
    bytes.extend(ciphertext);
    Ok(bytes)
}

/// Authenticate and decrypt a nonce-prefixed payload, rejecting truncated input.
pub fn open(key: &[u8; 32], bytes: &[u8]) -> Result<Vec<u8>, aes_gcm::Error> {
    if bytes.len() < 12 {
        return Err(aes_gcm::Error);
    }
    let (nonce, ciphertext) = bytes.split_at(12);
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| aes_gcm::Error)?;
    cipher.decrypt(
        Nonce::try_from(nonce).map_err(|_| aes_gcm::Error)?.as_ref(),
        ciphertext,
    )
}
