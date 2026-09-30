//! Importing connection settings from a JSON export document.

use crate::domain::types::DatabaseConnection;
use crate::storage::connections::{open_database, validate_name};
use crate::storage::crypto::{encrypt, unhex};
use crate::transfer::export::{EXPORT_KDF_ROUNDS, EXPORT_VERSION, ExportDocument};
use crate::ui::prompts::{Prompter, prompt_secret};
use anyhow::{Context, Result, ensure};
use dm_plugin_sdk::Context as PluginContext;
use dm_plugin_support::secrets;
use pbkdf2::pbkdf2_hmac;
use rusqlite::params;
use sha2::Sha256;

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
    let portable = match (
        document.connections,
        document.encrypted_payload,
        document.salt,
    ) {
        (Some(connections), None, None) => {
            ensure!(
                connections
                    .iter()
                    .all(|connection| connection.password.is_none()),
                "Plain exports cannot contain passwords; use an encrypted export"
            );
            connections
        }
        (None, Some(payload), Some(salt)) => {
            let passphrase = prompt_secret(prompter, "Import passphrase: ")?;
            let salt = unhex(&salt)?;
            ensure!(salt.len() == 16, "Invalid export salt length");
            let payload = unhex(&payload)?;
            ensure!(payload.len() >= 12, "Invalid encrypted export length");
            let mut key = [0_u8; 32];
            pbkdf2_hmac::<Sha256>(passphrase.as_bytes(), &salt, EXPORT_KDF_ROUNDS, &mut key);
            let plaintext = secrets::open(&key, &payload).map_err(|_| {
                anyhow::anyhow!("Import passphrase is incorrect or export is damaged")
            })?;
            serde_json::from_slice(&plaintext).context("Parse decrypted database connections")?
        }
        _ => anyhow::bail!(
            "Invalid export document: expected plain connections or an encrypted payload"
        ),
    };
    ensure!(
        portable.len() == document.count,
        "Export connection count does not match its contents"
    );
    let mut imported_names = std::collections::HashSet::new();
    for item in &portable {
        validate_name(&item.name)?;
        ensure!(
            item.port > 0,
            "Connection '{}' has an invalid port",
            item.name
        );
        ensure!(
            !item.host.trim().is_empty(),
            "Connection '{}' has an empty host",
            item.name
        );
        ensure!(
            !item.username.trim().is_empty(),
            "Connection '{}' has an empty username",
            item.name
        );
        ensure!(
            !item.driver.trim().is_empty(),
            "Connection '{}' has an empty driver",
            item.name
        );
        ensure!(
            imported_names.insert(&item.name),
            "Export contains duplicate connection '{}'",
            item.name
        );
    }
    let mut database = open_database(context)?;
    let transaction =
        database.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let mut statement = transaction.prepare("SELECT name, secret FROM connections")?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
    })?;
    let existing: std::collections::HashMap<String, Option<String>> =
        rows.collect::<rusqlite::Result<_>>()?;
    drop(statement);
    let collisions: Vec<_> = portable
        .iter()
        .filter(|connection| existing.contains_key(&connection.name))
        .map(|connection| connection.name.as_str())
        .collect();
    ensure!(
        replace || collisions.is_empty(),
        "Connections already exist: {}; pass --replace to overwrite",
        collisions.join(", ")
    );
    let mut prepared = Vec::with_capacity(portable.len());
    for item in portable {
        let secret = match item.password {
            Some(password) => Some(encrypt(context, password.as_bytes())?),
            None => existing.get(&item.name).and_then(|secret| secret.clone()),
        };
        prepared.push(DatabaseConnection {
            name: item.name,
            host: item.host,
            port: item.port,
            username: item.username,
            schema: item.schema,
            driver: item.driver,
            secret,
        });
    }
    for connection in &prepared {
        let sql = if replace {
            "INSERT INTO connections (name, host, port, username, schema, driver, secret)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(name) DO UPDATE SET host = excluded.host,
                 port = excluded.port, username = excluded.username,
                 schema = excluded.schema, driver = excluded.driver,
                 secret = excluded.secret, updated_at = unixepoch()"
        } else {
            "INSERT INTO connections (name, host, port, username, schema, driver, secret)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"
        };
        transaction.execute(
            sql,
            params![
                connection.name,
                connection.host,
                connection.port,
                connection.username,
                connection.schema,
                connection.driver,
                connection.secret
            ],
        )?;
    }
    transaction.commit()?;
    Ok(document.count)
}
