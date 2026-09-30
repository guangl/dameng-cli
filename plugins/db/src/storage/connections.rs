//! The connections.sqlite3 store behind add, list, remove and the commands
//! that talk to a saved connection.

use crate::domain::types::DatabaseConnection;
use crate::storage::crypto::decrypt;
use anyhow::{Context, Result, ensure};
use dm_plugin_sdk::Context as PluginContext;
use rusqlite::{Connection as Sqlite, params};
use std::{fs, path::PathBuf};

const CONNECTIONS_TABLE_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS connections (
    name TEXT PRIMARY KEY,
    host TEXT NOT NULL,
    port INTEGER NOT NULL DEFAULT 5236,
    username TEXT NOT NULL,
    schema TEXT,
    driver TEXT NOT NULL,
    secret TEXT,
    updated_at INTEGER NOT NULL DEFAULT (unixepoch())
) STRICT";

pub fn database_path(context: &PluginContext) -> PathBuf {
    context.data_dir.join("connections.sqlite3")
}

pub fn open_database(context: &PluginContext) -> Result<Sqlite> {
    fs::create_dir_all(&context.data_dir).context("Create the database plugin data directory")?;
    let connection =
        Sqlite::open(database_path(context)).context("Open the saved connections store")?;
    connection.execute_batch(CONNECTIONS_TABLE_SCHEMA)?;
    Ok(connection)
}

pub fn validate_name(name: &str) -> Result<()> {
    ensure!(
        !name.is_empty() && name.len() <= 64,
        "Connection name must contain 1–64 characters"
    );
    ensure!(
        name.bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')),
        "Connection name may only contain a-z, A-Z, 0-9, '-', '_' and '.'"
    );
    Ok(())
}

pub fn load_connections(context: &PluginContext) -> Result<Vec<DatabaseConnection>> {
    let connection = open_database(context)?;
    let mut statement = connection.prepare(
        "SELECT name, host, port, username, schema, driver, secret
         FROM connections ORDER BY name",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(DatabaseConnection {
            name: row.get(0)?,
            host: row.get(1)?,
            port: row.get(2)?,
            username: row.get(3)?,
            schema: row.get(4)?,
            driver: row.get(5)?,
            secret: row.get(6)?,
        })
    })?;
    rows.collect::<rusqlite::Result<_>>().map_err(Into::into)
}

pub fn upsert_connection(context: &PluginContext, connection: &DatabaseConnection) -> Result<()> {
    save_connection(context, connection, true)
}

pub(crate) fn save_connection(
    context: &PluginContext,
    connection: &DatabaseConnection,
    replace: bool,
) -> Result<()> {
    let database = open_database(context)?;
    let query = "INSERT INTO connections (name, host, port, username, schema, driver, secret)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(name) DO UPDATE SET host = excluded.host,
             port = excluded.port,
             username = excluded.username,
             schema = excluded.schema,
             driver = excluded.driver,
             secret = excluded.secret,
             updated_at = unixepoch()";
    let query = if replace {
        query
    } else {
        query.split("ON CONFLICT").next().unwrap_or(query)
    };
    database.execute(
        query,
        params![
            connection.name,
            connection.host,
            connection.port,
            connection.username,
            connection.schema,
            connection.driver,
            connection.secret,
        ],
    )?;
    Ok(())
}

pub fn remove_connection(context: &PluginContext, name: &str) -> Result<()> {
    let database = open_database(context)?;
    ensure!(
        database.execute("DELETE FROM connections WHERE name = ?1", [name])? == 1,
        "Database connection '{name}' is not configured"
    );
    Ok(())
}

pub fn find_connection(context: &PluginContext, name: &str) -> Result<DatabaseConnection> {
    load_connections(context)?
        .into_iter()
        .find(|connection| connection.name == name)
        .with_context(|| format!("Database connection '{name}' is not configured"))
}

/// Decrypt the password of a saved connection.
pub fn connection_password(
    context: &PluginContext,
    connection: &DatabaseConnection,
) -> Result<String> {
    let secret = connection
        .secret
        .as_deref()
        .context("The database password is not configured")?;
    let bytes = decrypt(context, secret)?;
    String::from_utf8(bytes).context("The stored database password is not valid UTF-8")
}
