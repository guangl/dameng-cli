//! Plain and passphrase-encrypted SSH server export documents.

use crate::crypto::{decrypt, hex};
use crate::prompts::{Prompter, prompt_export_passphrase};
use crate::servers::Server;
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

/// Implementation detail exposed for the tests; one server export.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
#[doc(hidden)]
pub struct ExportDocument {
    /// Implementation detail exposed for the tests; export format version.
    pub version: u32,
    /// Implementation detail exposed for the tests; number of servers.
    pub count: usize,
    /// Implementation detail exposed for the tests; plain servers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub servers: Option<Vec<PortableServer>>,
    /// Implementation detail exposed for the tests; hex nonce and ciphertext.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encrypted_payload: Option<String>,
    /// Implementation detail exposed for the tests; hex PBKDF2 salt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub salt: Option<String>,
}

/// Implementation detail exposed for the tests; one exported SSH server.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
#[doc(hidden)]
pub struct PortableServer {
    /// Implementation detail exposed for the tests; server name.
    pub name: String,
    /// Implementation detail exposed for the tests; SSH host.
    pub host: String,
    /// Implementation detail exposed for the tests; SSH port.
    pub port: u16,
    /// Implementation detail exposed for the tests; SSH user.
    pub username: String,
    /// Implementation detail exposed for the tests; "password" or "key".
    pub auth_type: String,
    /// Implementation detail exposed for the tests; only set for key authentication.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_path: Option<String>,
    /// Implementation detail exposed for the tests; only set when exported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
}

/// Implementation detail exposed for the tests; builds an export document.
#[doc(hidden)]
pub fn export_document(
    context: &PluginContext,
    servers: Vec<Server>,
    include_secrets: bool,
    prompter: Option<&dyn Prompter>,
) -> Result<ExportDocument> {
    let mut portable = Vec::with_capacity(servers.len());
    for server in servers {
        let secret = if include_secrets {
            server
                .secret
                .as_deref()
                .map(|secret| {
                    String::from_utf8(decrypt(context, secret)?)
                        .context("Saved SSH secret is not valid UTF-8")
                })
                .transpose()?
        } else {
            None
        };
        // Only key authentication carries a key path; a stale path on a password
        // server would make the document inconsistent for the importer.
        let key_path = (server.auth_type == "key")
            .then_some(server.key_path)
            .flatten();
        portable.push(PortableServer {
            name: server.name,
            host: server.host,
            port: server.port,
            username: server.username,
            auth_type: server.auth_type,
            key_path,
            secret,
        });
    }
    let count = portable.len();
    if !include_secrets {
        return Ok(ExportDocument {
            version: EXPORT_VERSION,
            count,
            servers: Some(portable),
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
        servers: None,
        encrypted_payload: Some(hex(&payload)),
        salt: Some(hex(&salt)),
    })
}
