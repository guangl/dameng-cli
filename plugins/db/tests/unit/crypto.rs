//! Tests for the machine key, the password encryption and the hex codec.

use crate::common::*;
use dm_plugin_db::{decrypt, encrypt, hex, machine_key, unhex};
use std::fs;
use tempfile::TempDir;

#[test]
fn hex_and_secret_round_trip() {
    let bytes = b"secret-value";
    assert_eq!(unhex(&hex(bytes)).unwrap(), bytes);

    let temp = TempDir::new().unwrap();
    let context = context(&temp);
    let encrypted = encrypt(&context, b"SYSDBA@123").unwrap();
    assert_ne!(encrypted, "SYSDBA@123");
    assert_eq!(decrypt(&context, &encrypted).unwrap(), b"SYSDBA@123");
}

#[test]
fn hex_rejects_invalid_input() {
    assert!(unhex("0").is_err());
    assert!(unhex("zz").is_err());
    let temp = TempDir::new().unwrap();
    let context = context(&temp);
    assert!(decrypt(&context, "aa").is_err());
}

#[test]
fn machine_key_rejects_invalid_and_unreadable_key_files() {
    let temp = TempDir::new().unwrap();
    let context = context(&temp);
    fs::create_dir_all(&context.data_dir).unwrap();
    fs::write(context.data_dir.join(".db-key"), b"too short").unwrap();
    assert!(machine_key(&context).is_err());

    fs::remove_file(context.data_dir.join(".db-key")).unwrap();
    fs::create_dir(context.data_dir.join(".db-key")).unwrap();
    assert!(machine_key(&context).is_err());
}

#[test]
fn decrypt_rejects_corrupt_ciphertext() {
    let temp = TempDir::new().unwrap();
    let context = context(&temp);
    let secret = encrypt(&context, b"SYSDBA@123").unwrap();
    let mut bytes = unhex(&secret).unwrap();
    bytes.truncate(12);
    bytes.extend_from_slice(&[0_u8; 16]);
    assert!(decrypt(&context, &hex(&bytes)).is_err());
}
