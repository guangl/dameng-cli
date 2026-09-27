use std::fs;

use dm_plugin_ssh::{decrypt, encrypt, hex, machine_key, unhex};
use tempfile::TempDir;

use crate::common::*;

#[test]
fn hex_round_trip() {
    let bytes = b"secret-value";
    assert_eq!(unhex(&hex(bytes)).unwrap(), bytes);
}

#[test]
fn encrypt_decrypt_round_trip() {
    let temp = TempDir::new().unwrap();
    let context = context(&temp);
    let encrypted = encrypt(&context, b"p@ssw0rd").unwrap();
    assert_ne!(encrypted, "p@ssw0rd");
    assert_eq!(decrypt(&context, &encrypted).unwrap(), b"p@ssw0rd");
}

#[test]
fn machine_key_rejects_invalid_key_length() {
    let temp = TempDir::new().unwrap();
    let context = context(&temp);
    fs::create_dir_all(&context.data_dir).unwrap();
    fs::write(context.data_dir.join(".ssh-key"), b"too short").unwrap();
    assert!(machine_key(&context).is_err());
}

#[test]
fn machine_key_rejects_unreadable_key() {
    let temp = TempDir::new().unwrap();
    let context = context(&temp);
    fs::create_dir_all(&context.data_dir).unwrap();
    fs::create_dir(context.data_dir.join(".ssh-key")).unwrap();
    assert!(machine_key(&context).is_err());
}

#[test]
fn decrypt_rejects_short_and_corrupt_text() {
    let temp = TempDir::new().unwrap();
    let context = context(&temp);
    assert!(decrypt(&context, "aa").is_err());
    let secret = encrypt(&context, b"p@ssw0rd").unwrap();
    let mut bytes = unhex(&secret).unwrap();
    bytes.truncate(12);
    bytes.extend_from_slice(&[0_u8; 16]);
    assert!(decrypt(&context, &hex(&bytes)).is_err());
}

#[test]
fn unhex_rejects_invalid_input() {
    assert!(unhex("0").is_err());
    assert!(unhex("zz").is_err());
}
