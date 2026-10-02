//! Machine-key encryption of stored passwords and the hexadecimal codec.

use anyhow::{Context, Result, ensure};
use dm_plugin_sdk::Context as PluginContext;
use dm_plugin_support::secrets;
use rand::{TryRng, rngs::SysRng};
use std::{fs, path::PathBuf};

pub fn key_path(context: &PluginContext) -> PathBuf {
    context.data_dir.join(".db-key")
}

/// Read or create the key that encrypts stored passwords.
pub fn machine_key(context: &PluginContext) -> Result<[u8; 32]> {
    fs::create_dir_all(&context.data_dir).context("Create the database plugin data directory")?;
    let path = key_path(context);
    if path.is_file() {
        let bytes = dm_plugin_support::bounded::file(&path, 32)
            .context("Read the connection encryption key")?;
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
    SysRng
        .try_fill_bytes(&mut key)
        .context("Generate cryptographic random bytes")?;
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
    let bytes = secrets::seal(&key, plaintext)
        .map_err(|_| anyhow::anyhow!("Encrypt the database password"))?;
    Ok(hex(&bytes))
}

pub fn decrypt(context: &PluginContext, text: &str) -> Result<Vec<u8>> {
    let bytes = unhex(text)?;
    ensure!(bytes.len() >= 12, "Invalid encrypted password length");
    let key = machine_key(context)?;
    secrets::open(&key, &bytes).map_err(|_| anyhow::anyhow!("Decrypt the database password"))
}

pub use dm_plugin_support::codec::{hex, unhex};
