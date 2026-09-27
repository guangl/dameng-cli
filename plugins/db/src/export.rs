//! Plain and passphrase-encrypted connection export documents.

use crate::crypto::{decrypt, hex};
use crate::prompts::{Prompter, prompt_export_passphrase};
use crate::types::DatabaseConnection;
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use anyhow::{Context, Result, ensure};
use dm_plugin_sdk::Context as PluginContext;
use pbkdf2::pbkdf2_hmac;
use rand::{RngCore, rngs::OsRng};
use sha2::Sha256;

/// Implementation detail exposed for the tests; export format version.
#[doc(hidden)]
pub const EXPORT_VERSION: u32 = 1;
/// PBKDF2 rounds applied to an encrypted export passphrase.
pub(crate) const EXPORT_KDF_ROUNDS: u32 = 600_000;

/// Implementation detail exposed for the tests; one connection export.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
#[doc(hidden)]
pub struct ExportDocument {
    /// Implementation detail exposed for the tests; export format version.
    pub version: u32,
    /// Implementation detail exposed for the tests; number of connections.
    pub count: usize,
    /// Implementation detail exposed for the tests; plain connections.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connections: Option<Vec<PortableConnection>>,
    /// Implementation detail exposed for the tests; hex nonce and ciphertext.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encrypted_payload: Option<String>,
    /// Implementation detail exposed for the tests; hex PBKDF2 salt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub salt: Option<String>,
}

/// Implementation detail exposed for the tests; one exported connection.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
#[doc(hidden)]
pub struct PortableConnection {
    /// Implementation detail exposed for the tests; connection name.
    pub name: String,
    /// Implementation detail exposed for the tests; database host.
    pub host: String,
    /// Implementation detail exposed for the tests; database port.
    pub port: u16,
    /// Implementation detail exposed for the tests; database user.
    pub username: String,
    /// Implementation detail exposed for the tests; selected schema.
    pub schema: Option<String>,
    /// Implementation detail exposed for the tests; driver name.
    pub driver: String,
    /// Implementation detail exposed for the tests; only set when exported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

/// Implementation detail exposed for the tests; builds an export document.
#[doc(hidden)]
pub fn export_document(
    context: &PluginContext,
    connections: Vec<DatabaseConnection>,
    include_passwords: bool,
    prompter: Option<&dyn Prompter>,
) -> Result<ExportDocument> {
    let mut portable = Vec::with_capacity(connections.len());
    for connection in connections {
        let password = if include_passwords {
            connection
                .secret
                .as_deref()
                .map(|secret| {
                    String::from_utf8(decrypt(context, secret)?)
                        .context("Saved password is not valid UTF-8")
                })
                .transpose()?
        } else {
            None
        };
        portable.push(PortableConnection {
            name: connection.name,
            host: connection.host,
            port: connection.port,
            username: connection.username,
            schema: connection.schema,
            driver: connection.driver,
            password,
        });
    }
    let count = portable.len();
    if !include_passwords {
        return Ok(ExportDocument {
            version: EXPORT_VERSION,
            count,
            connections: Some(portable),
            encrypted_payload: None,
            salt: None,
        });
    }
    let passphrase = prompt_export_passphrase(prompter)?;
    ensure!(
        !passphrase.is_empty(),
        "Export passphrase must not be empty"
    );
    let mut salt = [0_u8; 16];
    OsRng.fill_bytes(&mut salt);
    let mut key = [0_u8; 32];
    pbkdf2_hmac::<Sha256>(passphrase.as_bytes(), &salt, EXPORT_KDF_ROUNDS, &mut key);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    let mut nonce = [0_u8; 12];
    OsRng.fill_bytes(&mut nonce);
    let plaintext = serde_json::to_vec(&portable)?;
    let encrypted = cipher
        .encrypt(Nonce::from_slice(&nonce), plaintext.as_ref())
        .map_err(|_| anyhow::anyhow!("Encrypt the export"))?;
    let mut payload = nonce.to_vec();
    payload.extend(encrypted);
    Ok(ExportDocument {
        version: EXPORT_VERSION,
        count,
        connections: None,
        encrypted_payload: Some(hex(&payload)),
        salt: Some(hex(&salt)),
    })
}
