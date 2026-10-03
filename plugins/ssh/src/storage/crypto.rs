use anyhow::{Context, Result, ensure};
use dm_plugin_sdk::Context as PluginContext;
use dm_plugin_support::secrets;
use rand::{TryRng, rngs::SysRng};
use std::{fs, path::PathBuf};

pub fn key_path(context: &PluginContext) -> PathBuf {
    context.data_dir.join(".ssh-key")
}

pub fn machine_key(context: &PluginContext) -> Result<[u8; 32]> {
    fs::create_dir_all(&context.data_dir).context("Create SSH plugin data directory")?;
    let path = key_path(context);
    if path.is_file() {
        let bytes =
            dm_plugin_support::bounded::file(&path, 32).context("Read SSH encryption key")?;
        ensure!(
            bytes.len() == 32,
            "SSH encryption key is invalid; remove {} and retry",
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
    fs::write(&path, key).context("Write SSH encryption key")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(key)
}

pub fn encrypt(context: &PluginContext, plaintext: &[u8]) -> Result<String> {
    let key = machine_key(context)?;
    let bytes =
        secrets::seal(&key, plaintext).map_err(|_| anyhow::anyhow!("Encrypt SSH secret"))?;
    Ok(hex(&bytes))
}

pub fn decrypt(context: &PluginContext, text: &str) -> Result<Vec<u8>> {
    let bytes = unhex(text)?;
    ensure!(bytes.len() >= 12, "Invalid encrypted SSH secret length");
    let key = machine_key(context)?;
    secrets::open(&key, &bytes).map_err(|_| anyhow::anyhow!("Decrypt SSH secret"))
}

pub use dm_plugin_support::codec::{hex, unhex};
