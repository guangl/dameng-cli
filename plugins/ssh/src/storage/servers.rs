use anyhow::{Context, Result, ensure};
use dm_plugin_sdk::Context as PluginContext;
use rusqlite::{Connection, params};
use std::{fs, path::PathBuf};

pub struct Server {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_type: String,
    pub key_path: Option<String>,
    pub secret: Option<String>,
}

const SERVERS_TABLE_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS servers (
    name TEXT PRIMARY KEY,
    host TEXT NOT NULL,
    port INTEGER NOT NULL DEFAULT 22,
    username TEXT NOT NULL,
    auth_type TEXT NOT NULL DEFAULT 'password',
    secret TEXT,
    key_path TEXT,
    updated_at INTEGER NOT NULL DEFAULT (unixepoch())
) STRICT";

pub fn database_path(context: &PluginContext) -> PathBuf {
    context.data_dir.join("servers.sqlite3")
}

pub fn open_database(context: &PluginContext) -> Result<Connection> {
    fs::create_dir_all(&context.data_dir).context("Create SSH plugin data directory")?;
    let connection = Connection::open(database_path(context)).context("Open SSH servers store")?;
    connection.execute_batch(SERVERS_TABLE_SCHEMA)?;
    Ok(connection)
}

pub fn validate_name(name: &str) -> Result<()> {
    ensure!(
        !name.is_empty() && name.len() <= 64,
        "SSH server name must contain 1–64 characters"
    );
    ensure!(
        name.bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')),
        "SSH server name may only contain a-z, A-Z, 0-9, '-', '_' and '.'"
    );
    Ok(())
}

pub fn load_servers(context: &PluginContext) -> Result<Vec<Server>> {
    let connection = open_database(context)?;
    let mut statement = connection.prepare(
        "SELECT name, host, port, username, auth_type, key_path, secret
         FROM servers ORDER BY name",
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
    rows.collect::<rusqlite::Result<_>>().map_err(Into::into)
}

pub fn upsert_server(context: &PluginContext, server: &Server) -> Result<()> {
    save_server(context, server, true)
}

pub(crate) fn save_server(context: &PluginContext, server: &Server, replace: bool) -> Result<()> {
    let connection = open_database(context)?;
    let query = "INSERT INTO servers (name, host, port, username, auth_type, secret, key_path)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(name) DO UPDATE SET host = excluded.host,
             port = excluded.port,
             username = excluded.username,
             auth_type = excluded.auth_type,
             secret = excluded.secret,
             key_path = excluded.key_path,
             updated_at = unixepoch()";
    let query = if replace {
        query
    } else {
        query.split("ON CONFLICT").next().unwrap_or(query)
    };
    connection.execute(
        query,
        params![
            server.name,
            server.host,
            server.port,
            server.username,
            server.auth_type,
            server.secret,
            server.key_path,
        ],
    )?;
    Ok(())
}

pub fn remove_server(context: &PluginContext, name: &str) -> Result<()> {
    let connection = open_database(context)?;
    ensure!(
        connection.execute("DELETE FROM servers WHERE name = ?1", [name])? == 1,
        "SSH server '{name}' is not configured"
    );
    Ok(())
}
