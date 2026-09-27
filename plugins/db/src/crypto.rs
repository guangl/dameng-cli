//! Machine-key encryption of stored passwords and the hexadecimal codec.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use anyhow::{Context, Result, ensure};
use dm_plugin_sdk::Context as PluginContext;
use rand::{RngCore, rngs::OsRng};
use std::{fs, path::PathBuf};

pub fn key_path(context: &PluginContext) -> PathBuf {
    context.data_dir.join(".db-key")
}

/// Read or create the key that encrypts stored passwords.
pub fn machine_key(context: &PluginContext) -> Result<[u8; 32]> {
    fs::create_dir_all(&context.data_dir).context("Create the database plugin data directory")?;
    let path = key_path(context);
    if path.is_file() {
        let bytes = fs::read(&path).context("Read the connection encryption key")?;
        ensure!(
            bytes.len() == 32,
            "The connection encryption key is invalid; remove {} and retry",
            path.display()
        );
        let mut key = [0_u8; 32];
        key.copy_from_slice(&bytes);
        return Ok(key);
    }
    let mut key = [0_u8; 32];
    OsRng.fill_bytes(&mut key);
    fs::write(&path, key).context("Write the connection encryption key")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(key)
}

pub fn encrypt(context: &PluginContext, plaintext: &[u8]) -> Result<String> {
    let key = machine_key(context)?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    let mut nonce = [0_u8; 12];
    OsRng.fill_bytes(&mut nonce);
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), plaintext)
        .map_err(|_| anyhow::anyhow!("Encrypt the database password"))?;
    Ok(format!("{}{}", hex(&nonce), hex(&ciphertext)))
}

pub fn decrypt(context: &PluginContext, text: &str) -> Result<Vec<u8>> {
    let bytes = unhex(text)?;
    ensure!(bytes.len() >= 12, "Invalid encrypted password length");
    let (nonce, ciphertext) = bytes.split_at(12);
    let key = machine_key(context)?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    cipher
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|_| anyhow::anyhow!("Decrypt the database password"))
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn unhex(text: &str) -> Result<Vec<u8>> {
    ensure!(
        text.len() % 2 == 0 && text.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "Expected hexadecimal text"
    );
    (0..text.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&text[index..index + 2], 16).map_err(Into::into))
        .collect()
}
