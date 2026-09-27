//! Importing SSH server settings from a JSON export document.

mod records;

use crate::crypto::{encrypt, unhex};
use crate::export::{EXPORT_KDF_ROUNDS, EXPORT_VERSION, ExportDocument, PortableServer};
use crate::import::records::{retained_secret, validate};
use crate::prompts::{Prompter, prompt_secret};
use crate::servers::{Server, open_database};
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use anyhow::{Context, Result, ensure};
use dm_plugin_sdk::Context as PluginContext;
use pbkdf2::pbkdf2_hmac;
use rusqlite::params;
use sha2::Sha256;
use std::collections::HashMap;

/// Implementation detail exposed for the tests; imports an export document.
#[doc(hidden)]
pub fn import_document(
    context: &PluginContext,
    document: ExportDocument,
    replace: bool,
    prompter: Option<&dyn Prompter>,
) -> Result<usize> {
    ensure!(
        document.version == EXPORT_VERSION,
        "Unsupported export version {}",
        document.version
    );
    let portable = match (document.servers, document.encrypted_payload, document.salt) {
        (Some(servers), None, None) => {
            ensure!(
                servers.iter().all(|server| server.secret.is_none()),
                "Plain exports cannot contain secrets; use an encrypted export"
            );
            servers
        }
        (None, Some(payload), Some(salt)) => decrypt_payload(&payload, &salt, prompter)?,
        _ => {
            anyhow::bail!("Invalid export document: expected plain servers or an encrypted payload")
        }
    };
    ensure!(
        portable.len() == document.count,
        "Export server count does not match its contents"
    );
    validate(&portable)?;
    let mut database = open_database(context)?;
    let transaction =
        database.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let existing = load_existing(&transaction)?;
    let collisions: Vec<_> = portable
        .iter()
        .filter(|server| existing.contains_key(&server.name))
        .map(|server| server.name.as_str())
        .collect();
    ensure!(
        replace || collisions.is_empty(),
        "SSH servers already exist: {}; pass --replace to overwrite",
        collisions.join(", ")
    );
    let mut prepared = Vec::with_capacity(portable.len());
    for item in portable {
        let secret = match item.secret {
            Some(secret) => Some(encrypt(context, secret.as_bytes())?),
            None => retained_secret(existing.get(&item.name), &item),
        };
        let key_path = (item.auth_type == "key").then_some(item.key_path).flatten();
        prepared.push(Server {
            name: item.name,
            host: item.host.trim().to_owned(),
            port: item.port,
            username: item.username.trim().to_owned(),
            auth_type: item.auth_type,
            key_path,
            secret,
        });
    }
    for server in &prepared {
        transaction.execute(
            insert_sql(replace),
            params![
                server.name,
                server.host,
                server.port,
                server.username,
                server.auth_type,
                server.secret,
                server.key_path
            ],
        )?;
    }
    transaction.commit()?;
    Ok(document.count)
}

/// Decrypt a passphrase-protected export into its portable servers.
fn decrypt_payload(
    payload: &str,
    salt: &str,
    prompter: Option<&dyn Prompter>,
) -> Result<Vec<PortableServer>> {
    let passphrase = prompt_secret(prompter, "Import passphrase: ")?;
    let salt = unhex(salt)?;
    ensure!(salt.len() == 16, "Invalid export salt length");
    let payload = unhex(payload)?;
    ensure!(payload.len() >= 12, "Invalid encrypted export length");
    let mut key = [0_u8; 32];
    pbkdf2_hmac::<Sha256>(passphrase.as_bytes(), &salt, EXPORT_KDF_ROUNDS, &mut key);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    let (nonce, ciphertext) = payload.split_at(12);
    let plaintext = cipher
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|_| anyhow::anyhow!("Import passphrase is incorrect or export is damaged"))?;
    serde_json::from_slice(&plaintext).context("Parse decrypted SSH servers")
}

fn load_existing(transaction: &rusqlite::Transaction<'_>) -> Result<HashMap<String, Server>> {
    let mut statement = transaction.prepare(
        "SELECT name, host, port, username, auth_type, key_path, secret
         FROM servers",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(Server {
            name: row.get(0)?,
            host: row.get(1)?,
            port: row.get(2)?,
            username: row.get(3)?,
            auth_type: row.get(4)?,
            key_path: row.get(5)?,
            secret: row.get(6)?,
        })
    })?;
    Ok(rows
        .collect::<rusqlite::Result<Vec<_>>>()?
        .into_iter()
        .map(|server| (server.name.clone(), server))
        .collect())
}

fn insert_sql(replace: bool) -> &'static str {
    if replace {
        "INSERT INTO servers (name, host, port, username, auth_type, secret, key_path)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(name) DO UPDATE SET host = excluded.host,
             port = excluded.port, username = excluded.username,
             auth_type = excluded.auth_type, secret = excluded.secret,
             key_path = excluded.key_path, updated_at = unixepoch()"
    } else {
        "INSERT INTO servers (name, host, port, username, auth_type, secret, key_path)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"
    }
}
