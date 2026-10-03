use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use dm_plugin_support::secrets::{open, seal};

#[test]
fn reads_existing_nonce_prefixed_ciphertext() {
    let key = [42; 32];
    let nonce = [7; 12];
    let cipher = Aes256Gcm::new_from_slice(&key).unwrap();
    let mut payload = nonce.to_vec();
    payload.extend(
        cipher
            .encrypt(&Nonce::from(nonce), b"existing secret".as_ref())
            .unwrap(),
    );
    assert_eq!(open(&key, &payload).unwrap(), b"existing secret");
}

#[test]
fn fresh_ciphertext_remains_readable_by_the_previous_format() {
    let key = [42; 32];
    let bytes = seal(&key, b"secret").unwrap();
    let cipher = Aes256Gcm::new_from_slice(&key).unwrap();
    assert_eq!(
        cipher
            .decrypt(&Nonce::try_from(&bytes[..12]).unwrap(), &bytes[12..])
            .unwrap(),
        b"secret"
    );
    assert_ne!(bytes, seal(&key, b"secret").unwrap());
}

#[test]
fn rejects_truncation_wrong_keys_and_tampering() {
    assert!(open(&[0; 32], &[0; 11]).is_err());
    assert!(open(&[0; 32], &[0; 12]).is_err());
    let mut bytes = seal(&[42; 32], b"secret").unwrap();
    assert!(open(&[0; 32], &bytes).is_err());
    *bytes.last_mut().unwrap() ^= 1;
    assert!(open(&[42; 32], &bytes).is_err());
}
