//! dm db: save Dameng database connections.
//!
//! The plugin owns everything below its own directories: connections live in
//! connections.sqlite3 inside the plugin data directory, with the password
//! encrypted by a machine key next to it, and its settings live in config.toml
//! inside the plugin config directory. The host only creates those directories
//! and passes their paths.
//!
//! Saving, listing and removing connections never talk to a database. The
//! Database/Session/DatabaseFactory interfaces and the "dm db test" and
//! "dm db exec" commands exist, but the driver behind them is still a
//! placeholder that reports it is not implemented; see src/driver.rs. Run
//! "dm info db" to see where the configuration file belongs.

pub mod driver;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use anyhow::{Context, Error, Result, ensure};
use clap::{Parser, Subcommand};
use dm_plugin_sdk::{Context as PluginContext, Plugin, PluginResult};
use driver::PendingFactory;
use pbkdf2::pbkdf2_hmac;
use rand::{RngCore, rngs::OsRng};
use rusqlite::{Connection as Sqlite, params};
use serde::Deserialize;
use sha2::Sha256;
use std::{
    ffi::OsString,
    fs,
    io::{IsTerminal, Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

/// Port used when --port is omitted and the prompt is answered with Enter.
pub const DEFAULT_PORT: u16 = 5236;
/// Database user used when --username is omitted.
pub const DEFAULT_USERNAME: &str = "SYSDBA";
/// Driver name recorded for a connection when --driver is omitted.
pub const DEFAULT_DRIVER: &str = "DM8 ODBC DRIVER";
/// SQL run by "dm db test" when the configuration does not name another probe.
pub const DEFAULT_PROBE: &str = "SELECT 1";
/// Login timeout in seconds for "dm db test" and "dm db exec".
pub const DEFAULT_LOGIN_TIMEOUT: u64 = 10;

/// A saved connection. The secret holds the AES-GCM encrypted password.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseConnection {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub schema: Option<String>,
    pub driver: String,
    pub secret: Option<String>,
}

/// Everything a driver needs for one connect attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionSpec {
    /// Connection string handed to the driver, including the password.
    pub connection_string: String,
    pub login_timeout: Duration,
}

/// A materialised result set; a None value is SQL NULL.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Option<String>>>,
}

/// What one statement produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The statement returned a result set.
    Rows(QueryResult),
    /// The statement returned no result set; the value is the affected row count.
    Affected(u64),
}

/// A connected database.
pub trait Database {
    fn connect(&self, spec: &ConnectionSpec) -> Result<Box<dyn Session + '_>>;
}

/// One connection, reused for every statement of a command.
pub trait Session {
    fn run(&mut self, sql: &str) -> Result<Outcome>;
}

/// Loads the driver a command should talk to.
///
/// Commands that never touch a database (add, list, remove) do not call this, so
/// they keep working without any driver implementation at all.
pub trait DatabaseFactory {
    fn open(&self) -> Result<Box<dyn Database>>;
}

pub struct DbPlugin;

impl Plugin for DbPlugin {
    fn run(&self, context: PluginContext) -> PluginResult {
        match run_cli(&context) {
            Ok(code) => Ok(code),
            Err(error) => {
                eprintln!("dm db: {error:#}");
                eprintln!("提示：{}", db_hint(&error));
                Ok(1)
            }
        }
    }
}

#[derive(Parser)]
#[command(
    name = "dm db",
    about = "Manage saved Dameng database connections",
    after_help = "This plugin reads its own configuration file (<config dir>/config.toml, see dm info db):\n  [defaults] port, username, driver, schema\n  [connect] timeout, probe"
)]
struct Cli {
    #[command(subcommand)]
    command: DbCommand,
}

#[derive(Subcommand)]
enum DbCommand {
    /// Add or replace a saved connection; prompts interactively for omitted values.
    Add {
        /// Connection name (prompted when omitted on a terminal).
        name: Option<String>,
        /// Database host (prompted when omitted on a terminal).
        #[arg(long)]
        host: Option<String>,
        /// Database port, default 5236 (prompted when omitted on a terminal).
        #[arg(long)]
        port: Option<u16>,
        /// Database user (prompted when omitted on a terminal).
        #[arg(long)]
        username: Option<String>,
        /// Password of the database user; prompted when omitted on a terminal.
        #[arg(long)]
        password: Option<String>,
        /// Schema to select after connecting (prompted when omitted on a terminal).
        #[arg(long)]
        schema: Option<String>,
        /// Driver name for this connection, default "DM8 ODBC DRIVER".
        #[arg(long)]
        driver: Option<String>,
    },
    /// List saved connections.
    List,
    /// Remove a saved connection.
    Remove { name: String },
    /// Export connection settings. Passwords are omitted unless encrypted export is requested.
    Export {
        #[arg(long)]
        file: Option<PathBuf>,
        /// Include passwords encrypted with an export passphrase.
        #[arg(long)]
        include_passwords: bool,
    },
    /// Import connection settings from a JSON export.
    Import {
        file: PathBuf,
        /// Replace connections with matching names.
        #[arg(long)]
        replace: bool,
    },
    /// Test a saved connection through the driver (not implemented yet).
    Test { name: String },
    /// Run SQL on a saved connection; reads stdin when no SQL and no --file is given.
    Exec {
        name: String,
        /// SQL text to run.
        sql: Option<String>,
        /// Read the SQL from this file instead.
        #[arg(long)]
        file: Option<PathBuf>,
    },
}

/// The values of "dm db add" before defaults and prompts are applied.
struct AddRequest {
    name: Option<String>,
    host: Option<String>,
    port: Option<u16>,
    username: Option<String>,
    password: Option<String>,
    schema: Option<String>,
    driver: Option<String>,
}

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

const EXPORT_VERSION: u32 = 1;
const EXPORT_KDF_ROUNDS: u32 = 600_000;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportDocument {
    version: u32,
    count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    connections: Option<Vec<PortableConnection>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    encrypted_payload: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    salt: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct PortableConnection {
    name: String,
    host: String,
    port: u16,
    username: String,
    schema: Option<String>,
    driver: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    password: Option<String>,
}

fn export_document(
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
    let passphrase = prompt_secret(prompter, "Export passphrase: ")?;
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

fn import_document(
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
            let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
            let (nonce, ciphertext) = payload.split_at(12);
            let plaintext = cipher
                .decrypt(Nonce::from_slice(nonce), ciphertext)
                .map_err(|_| {
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
            imported_names.insert(&item.name),
            "Export contains duplicate connection '{}'",
            item.name
        );
    }
    let existing: std::collections::HashMap<String, DatabaseConnection> =
        load_connections(context)?
            .into_iter()
            .map(|connection| (connection.name.clone(), connection))
            .collect();
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
            None => existing
                .get(&item.name)
                .and_then(|connection| connection.secret.clone()),
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
    let mut database = open_database(context)?;
    let transaction = database.transaction()?;
    for connection in &prepared {
        transaction.execute(
            "INSERT INTO connections (name, host, port, username, schema, driver, secret)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(name) DO UPDATE SET host = excluded.host,
                 port = excluded.port, username = excluded.username,
                 schema = excluded.schema, driver = excluded.driver,
                 secret = excluded.secret, updated_at = unixepoch()",
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

fn prompt_secret(prompter: Option<&dyn Prompter>, prompt: &str) -> Result<String> {
    prompter
        .context("A terminal is required for encrypted import/export passphrases")?
        .secret(prompt)
}

fn write_private_file(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("Create {}", parent.display()))?;
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut output = options.open(path).with_context(|| {
        format!(
            "Create export file {} without overwriting an existing file",
            path.display()
        )
    })?;
    output
        .write_all(bytes)
        .with_context(|| format!("Write {}", path.display()))?;
    Ok(())
}

pub fn database_path(context: &PluginContext) -> PathBuf {
    context.data_dir.join("connections.sqlite3")
}

pub fn key_path(context: &PluginContext) -> PathBuf {
    context.data_dir.join(".db-key")
}

pub fn open_database(context: &PluginContext) -> Result<Sqlite> {
    fs::create_dir_all(&context.data_dir).context("Create the database plugin data directory")?;
    let connection =
        Sqlite::open(database_path(context)).context("Open the saved connections store")?;
    connection.execute_batch(CONNECTIONS_TABLE_SCHEMA)?;
    Ok(connection)
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
    let database = open_database(context)?;
    database.execute(
        "INSERT INTO connections (name, host, port, username, schema, driver, secret)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(name) DO UPDATE SET host = excluded.host,
             port = excluded.port,
             username = excluded.username,
             schema = excluded.schema,
             driver = excluded.driver,
             secret = excluded.secret,
             updated_at = unixepoch()",
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

/// Settings read from the plugin's own config.toml in its config directory.
///
/// The host only creates and passes the directory; the schema below belongs to
/// the plugin, so these keys never appear in the host configuration file.
#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DbConfig {
    /// Values used by "dm db add" when a flag is omitted.
    #[serde(default)]
    pub defaults: DbDefaults,
    /// How "dm db test" and "dm db exec" connect.
    #[serde(default)]
    pub connect: DbConnect,
}

/// The \[defaults\] table.
#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DbDefaults {
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub driver: Option<String>,
    #[serde(default)]
    pub schema: Option<String>,
}

/// The \[connect\] table.
#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DbConnect {
    #[serde(default)]
    pub timeout: Option<u64>,
    #[serde(default)]
    pub probe: Option<String>,
}

/// Path of the plugin's own configuration file.
pub fn config_path(context: &PluginContext) -> PathBuf {
    context.config_file()
}

/// Load the plugin configuration. A missing file means "all defaults".
pub fn load_config(context: &PluginContext) -> Result<DbConfig> {
    let path = config_path(context);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(DbConfig::default());
        }
        Err(error) => return Err(error).with_context(|| format!("Read {}", path.display())),
    };
    let config: DbConfig = toml::from_str(&text).with_context(|| {
        format!(
            "Invalid database plugin configuration {}; supported tables are [defaults] and [connect]",
            path.display()
        )
    })?;
    if let Some(port) = config.defaults.port {
        ensure!(
            port > 0,
            "Database plugin configuration {}: defaults.port must not be zero",
            path.display()
        );
    }
    if let Some(timeout) = config.connect.timeout {
        ensure!(
            timeout > 0,
            "Database plugin configuration {}: connect.timeout must be greater than zero",
            path.display()
        );
    }
    for (key, value) in [
        ("defaults.username", config.defaults.username.as_deref()),
        ("defaults.driver", config.defaults.driver.as_deref()),
        ("defaults.schema", config.defaults.schema.as_deref()),
        ("connect.probe", config.connect.probe.as_deref()),
    ] {
        ensure!(
            !value.is_some_and(|value| value.trim().is_empty()),
            "Database plugin configuration {}: {key} must not be empty",
            path.display()
        );
    }
    Ok(config)
}

/// Source of interactive answers.
///
/// Production prompts the controlling terminal; tests script the answers so the
/// interactive branches stay covered without a real TTY.
pub trait Prompter {
    /// Read one visible line, trimming surrounding whitespace.
    fn line(&self, prompt: &str) -> Result<String>;
    /// Read one hidden line, used for passwords.
    fn secret(&self, prompt: &str) -> Result<String>;
}

/// Ask the user on the terminal: the prompt goes to stdout so it appears before
/// the answer is read from stdin.
pub struct TerminalPrompter;

impl Prompter for TerminalPrompter {
    fn line(&self, prompt: &str) -> Result<String> {
        use std::io::Write;
        print!("{prompt}");
        std::io::stdout().flush().context("Flush prompt")?;
        let mut input = String::new();
        std::io::stdin()
            .read_line(&mut input)
            .context("Read input")?;
        Ok(input.trim().to_owned())
    }

    fn secret(&self, prompt: &str) -> Result<String> {
        rpassword::prompt_password(prompt).context("Read hidden input")
    }
}

/// Use the terminal prompter only when stdin is attached to a terminal.
fn terminal_prompter() -> Option<&'static dyn Prompter> {
    static TERMINAL: TerminalPrompter = TerminalPrompter;
    std::io::stdin().is_terminal().then_some(&TERMINAL)
}

/// Resolve a required plain-text field, prompting when it was omitted and a
/// prompter is available (None means stdin is not a terminal).
pub fn resolve_required(
    value: Option<String>,
    prompt: &str,
    missing: &str,
    prompter: Option<&dyn Prompter>,
) -> Result<String> {
    match (value, prompter) {
        (Some(value), _) => Ok(value),
        (None, Some(prompter)) => prompter.line(prompt),
        (None, None) => anyhow::bail!("{missing}"),
    }
}

/// Resolve a value that falls back to the plugin configuration and then to a
/// documented default; an empty interactive answer keeps that default.
pub fn resolve_with_default(
    value: Option<String>,
    configured: Option<String>,
    fallback: &str,
    prompt: &str,
    prompter: Option<&dyn Prompter>,
) -> Result<String> {
    let value = match (value, configured) {
        (Some(value), _) => value,
        (None, Some(configured)) => configured,
        (None, None) => match prompter {
            Some(prompter) => {
                let answer = prompter.line(&format!("{prompt} [{fallback}]: "))?;
                if answer.is_empty() {
                    fallback.to_owned()
                } else {
                    answer
                }
            }
            None => fallback.to_owned(),
        },
    };
    ensure!(!value.trim().is_empty(), "A value must not be empty");
    Ok(value)
}

/// Resolve an optional value; an empty answer means "not set".
pub fn resolve_optional(
    value: Option<String>,
    prompt: &str,
    prompter: Option<&dyn Prompter>,
) -> Result<Option<String>> {
    let value = match (value, prompter) {
        (Some(value), _) => Some(value),
        (None, Some(prompter)) => Some(prompter.line(prompt)?),
        (None, None) => None,
    };
    Ok(value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty()))
}

/// Resolve the port, defaulting to 5236 when it is not configured.
pub fn resolve_port(
    port: Option<u16>,
    configured: Option<u16>,
    prompter: Option<&dyn Prompter>,
) -> Result<u16> {
    match (port, configured) {
        (Some(port), _) => Ok(port),
        (None, Some(port)) => Ok(port),
        (None, None) => match prompter {
            Some(prompter) => {
                let value = prompter.line(&format!("Port [{DEFAULT_PORT}]: "))?;
                if value.is_empty() {
                    Ok(DEFAULT_PORT)
                } else {
                    value
                        .parse::<u16>()
                        .with_context(|| format!("Database port must be a number, got '{value}'"))
                }
            }
            None => Ok(DEFAULT_PORT),
        },
    }
}

/// Resolve the password for "dm db add", prompting on the terminal when one was
/// not supplied and the process is attached to a terminal.
pub fn resolve_password(
    password: Option<String>,
    prompter: Option<&dyn Prompter>,
) -> Result<String> {
    match password {
        Some(password) => {
            ensure!(!password.is_empty(), "Database password must not be empty");
            Ok(password)
        }
        None => match prompter {
            Some(prompter) => {
                let password = prompter
                    .secret("Password: ")
                    .context("Read the database password")?;
                ensure!(!password.is_empty(), "Database password must not be empty");
                Ok(password)
            }
            None => anyhow::bail!(
                "Database password is required; pass --password or run from a terminal"
            ),
        },
    }
}

/// Quote a connection-string value when it contains a separator.
fn quote_value(value: &str) -> String {
    if value.contains([';', '{', '}', '=']) || value.trim() != value {
        format!("{{{}}}", value.replace('}', "}}"))
    } else {
        value.to_owned()
    }
}

fn reject_separators(label: &str, value: &str) -> Result<()> {
    ensure!(
        !value.contains([';', '{', '}']),
        "Database {label} must not contain ';', '{{' or '}}'"
    );
    Ok(())
}

/// Build the connection string used by "dm db test" and "dm db exec".
///
/// The keywords follow the ODBC connection-string syntax the Dameng ODBC driver
/// expects; the driver itself is still deferred, see src/driver.rs.
pub fn build_connection_string(connection: &DatabaseConnection, password: &str) -> Result<String> {
    let host = connection.host.trim();
    let username = connection.username.trim();
    let driver = connection.driver.trim();
    ensure!(!host.is_empty(), "Database host must not be empty");
    ensure!(!username.is_empty(), "Database username must not be empty");
    ensure!(!driver.is_empty(), "The driver name must not be empty");
    reject_separators("host", host)?;
    reject_separators("username", username)?;
    let mut parts = vec![
        format!("Driver={{{driver}}}"),
        format!("Server={host}"),
        format!("Port={}", connection.port),
        format!("UID={username}"),
        format!("PWD={}", quote_value(password)),
    ];
    if let Some(schema) = connection
        .schema
        .as_deref()
        .map(str::trim)
        .filter(|schema| !schema.is_empty())
    {
        reject_separators("schema", schema)?;
        parts.push(format!("Schema={schema}"));
    }
    Ok(format!("{};", parts.join(";")))
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

fn connection_spec(
    connection: &DatabaseConnection,
    password: &str,
    config: &DbConfig,
) -> Result<ConnectionSpec> {
    Ok(ConnectionSpec {
        connection_string: build_connection_string(connection, password)?,
        login_timeout: Duration::from_secs(config.connect.timeout.unwrap_or(DEFAULT_LOGIN_TIMEOUT)),
    })
}

/// Read the SQL of "dm db exec" from the argument, a file, or stdin.
pub fn read_sql(sql: Option<String>, file: Option<PathBuf>) -> Result<String> {
    let text = match (sql, file) {
        (Some(sql), _) => sql,
        (None, Some(path)) => {
            fs::read_to_string(&path).with_context(|| format!("Read {}", path.display()))?
        }
        (None, None) => {
            let mut text = String::new();
            std::io::stdin()
                .read_to_string(&mut text)
                .context("Read SQL from stdin")?;
            text
        }
    };
    let text = text.trim().to_owned();
    ensure!(
        !text.is_empty(),
        "SQL must not be empty; pass it as an argument, through --file, or on stdin"
    );
    Ok(text)
}

/// Render a result set as tab-separated rows; SQL NULL becomes an empty field.
pub fn format_result(result: &QueryResult) -> String {
    let mut output = String::new();
    output.push_str(&result.columns.join("\t"));
    output.push('\n');
    for row in &result.rows {
        let values: Vec<&str> = row
            .iter()
            .map(|value| value.as_deref().unwrap_or(""))
            .collect();
        output.push_str(&values.join("\t"));
        output.push('\n');
    }
    output
}

pub fn run_cli(context: &PluginContext) -> Result<i32> {
    run_with_prompter(context, &PendingFactory, terminal_prompter())
}

/// Command dispatch without prompts, used by tests and scripts.
pub fn run_with(context: &PluginContext, factory: &dyn DatabaseFactory) -> Result<i32> {
    run_with_prompter(context, factory, None)
}

/// Command dispatch with an injectable driver and prompt source, so tests cover
/// every branch without a terminal or a database.
pub fn run_with_prompter(
    context: &PluginContext,
    factory: &dyn DatabaseFactory,
    prompter: Option<&dyn Prompter>,
) -> Result<i32> {
    let mut argv = vec![OsString::from("dm db")];
    argv.extend(context.args.iter().cloned());
    let cli = Cli::parse_from(argv);
    match cli.command {
        DbCommand::Add {
            name,
            host,
            port,
            username,
            password,
            schema,
            driver,
        } => {
            let name = add_connection(
                context,
                &AddRequest {
                    name,
                    host,
                    port,
                    username,
                    password,
                    schema,
                    driver,
                },
                prompter,
            )?;
            println!("Saved database connection {name}");
        }
        DbCommand::List => {
            for connection in load_connections(context)? {
                println!(
                    "{}\t{}@{}:{}\t{}\t{}",
                    connection.name,
                    connection.username,
                    connection.host,
                    connection.port,
                    connection.schema.as_deref().unwrap_or("-"),
                    connection.driver
                );
            }
        }
        DbCommand::Remove { name } => {
            remove_connection(context, &name)?;
            println!("Removed database connection {name}");
        }
        DbCommand::Export {
            file,
            include_passwords,
        } => {
            let connections = load_connections(context)?;
            let document = export_document(context, connections, include_passwords, prompter)?;
            let json = serde_json::to_vec_pretty(&document)?;
            match file {
                Some(path) => {
                    write_private_file(&path, &json)?;
                    println!(
                        "Exported {} database connections to {}",
                        document.count,
                        path.display()
                    );
                }
                None => {
                    std::io::stdout().write_all(&json)?;
                    println!();
                }
            }
        }
        DbCommand::Import { file, replace } => {
            let document: ExportDocument = serde_json::from_slice(
                &fs::read(&file).with_context(|| format!("Read {}", file.display()))?,
            )
            .with_context(|| format!("Parse database connection export {}", file.display()))?;
            let connections = import_document(context, document, replace, prompter)?;
            println!("Imported {connections} database connections");
        }
        DbCommand::Test { name } => {
            let config = load_config(context)?;
            let connection = find_connection(context, &name)?;
            let password = connection_password(context, &connection)?;
            let spec = connection_spec(&connection, &password, &config)?;
            let database = factory.open()?;
            let mut session = database.connect(&spec)?;
            session.run(config.connect.probe.as_deref().unwrap_or(DEFAULT_PROBE))?;
            println!("Database connection {name} is reachable");
        }
        DbCommand::Exec { name, sql, file } => {
            // The SQL is read before anything else so an empty statement fails
            // fast and without touching the store or the driver.
            let sql = read_sql(sql, file)?;
            let config = load_config(context)?;
            let connection = find_connection(context, &name)?;
            let password = connection_password(context, &connection)?;
            let spec = connection_spec(&connection, &password, &config)?;
            let database = factory.open()?;
            let mut session = database.connect(&spec)?;
            match session.run(&sql)? {
                Outcome::Rows(result) => print!("{}", format_result(&result)),
                Outcome::Affected(count) => println!("{count} rows affected"),
            }
        }
    }
    Ok(0)
}

/// Collect the values of "dm db add" from flags, the plugin configuration, prompts.
fn add_connection(
    context: &PluginContext,
    request: &AddRequest,
    prompter: Option<&dyn Prompter>,
) -> Result<String> {
    let config = load_config(context)?;
    let name = resolve_required(
        request.name.clone(),
        "Name: ",
        "Connection name is required",
        prompter,
    )?;
    validate_name(&name)?;
    let host = resolve_required(
        request.host.clone(),
        "Host: ",
        "Database host is required",
        prompter,
    )?;
    ensure!(!host.trim().is_empty(), "Database host must not be empty");
    let port = resolve_port(request.port.or(config.defaults.port), None, prompter)?;
    let username = resolve_with_default(
        request.username.clone(),
        config.defaults.username.clone(),
        DEFAULT_USERNAME,
        "Username: ",
        prompter,
    )?;
    let driver = resolve_with_default(
        request.driver.clone(),
        config.defaults.driver.clone(),
        DEFAULT_DRIVER,
        "Driver: ",
        prompter,
    )?;
    let schema = resolve_optional(
        request.schema.clone().or(config.defaults.schema.clone()),
        "Schema (leave empty for the login default): ",
        prompter,
    )?;
    let password = resolve_password(request.password.clone(), prompter)?;
    upsert_connection(
        context,
        &DatabaseConnection {
            name: name.clone(),
            host: host.trim().to_owned(),
            port,
            username: username.trim().to_owned(),
            schema,
            driver: driver.trim().to_owned(),
            secret: Some(encrypt(context, password.as_bytes())?),
        },
    )?;
    Ok(name)
}

/// Return a short, actionable hint for a database plugin error.
#[doc(hidden)]
pub fn db_hint(error: &Error) -> String {
    let text = error
        .chain()
        .map(|cause| cause.to_string())
        .collect::<Vec<_>>()
        .join("\n")
        .to_lowercase();

    if text.contains("is not configured") {
        return "请先运行 dm db add <name> 配置连接，或用 dm db list 查看已保存的连接。".into();
    }
    if text.contains("driver is not implemented") {
        return "数据库驱动尚未接入：当前 dm db 只管理连接配置，dm db test 与 dm db exec 会在驱动实现后可用。".into();
    }
    if text.contains("must not be empty") || text.contains(" is required") {
        return "缺少必填项；在终端下运行可交互输入，或显式传入对应参数。".into();
    }
    if text.contains("no such table")
        || text.contains("no such column")
        || text.contains("no column named")
        || text.contains("sql logic error")
        || text.contains("store")
        || text.contains("sqlite")
    {
        return "连接存储异常，请检查插件数据目录中的 connections.sqlite3。".into();
    }

    "使用 dm db --help 查看可用子命令和参数。".into()
}

#[cfg(test)]
mod export_tests {
    use super::*;
    use std::cell::RefCell;
    use tempfile::TempDir;

    struct PasswordPrompter(RefCell<Vec<String>>);

    impl Prompter for PasswordPrompter {
        fn line(&self, _prompt: &str) -> Result<String> {
            anyhow::bail!("Unexpected text prompt")
        }

        fn secret(&self, _prompt: &str) -> Result<String> {
            self.0
                .borrow_mut()
                .pop()
                .ok_or_else(|| anyhow::anyhow!("No scripted password"))
        }
    }

    fn context(temp: &TempDir, id: &str) -> PluginContext {
        let home = temp.path().join(id);
        PluginContext {
            args: vec![],
            plugin_dir: home.join("plugins/db"),
            config_dir: home.join("config/db"),
            data_dir: home.join("data/db"),
            cache_dir: home.join("cache/db"),
            home,
            capabilities: vec![],
        }
    }

    fn connection(
        name: &str,
        password: Option<&str>,
        context: &PluginContext,
    ) -> DatabaseConnection {
        DatabaseConnection {
            name: name.to_owned(),
            host: "127.0.0.1".to_owned(),
            port: DEFAULT_PORT,
            username: DEFAULT_USERNAME.to_owned(),
            schema: Some("DMHR".to_owned()),
            driver: DEFAULT_DRIVER.to_owned(),
            secret: password.map(|value| encrypt(context, value.as_bytes()).unwrap()),
        }
    }

    fn prompter(password: &str) -> PasswordPrompter {
        PasswordPrompter(RefCell::new(vec![password.to_owned()]))
    }

    #[test]
    fn plain_export_omits_passwords_and_round_trips_configuration() {
        let temp = TempDir::new().unwrap();
        let source = context(&temp, "source");
        let export = export_document(
            &source,
            vec![connection("prod", Some("secret"), &source)],
            false,
            None,
        )
        .unwrap();
        let json = serde_json::to_string(&export).unwrap();
        assert!(!json.contains("secret"));
        let imported: ExportDocument = serde_json::from_str(&json).unwrap();
        let target = context(&temp, "target");
        assert_eq!(import_document(&target, imported, false, None).unwrap(), 1);
        let saved = find_connection(&target, "prod").unwrap();
        assert_eq!(saved.host, "127.0.0.1");
        assert!(saved.secret.is_none());
    }

    #[test]
    fn encrypted_export_import_reencrypts_password_for_destination() {
        let temp = TempDir::new().unwrap();
        let source = context(&temp, "source");
        let target = context(&temp, "target");
        let source_connection = connection("prod", Some("secret"), &source);
        let source_secret = source_connection.secret.clone().unwrap();
        let encrypted = export_document(
            &source,
            vec![source_connection],
            true,
            Some(&prompter("transfer-passphrase")),
        )
        .unwrap();
        let json = serde_json::to_string(&encrypted).unwrap();
        assert!(!json.contains("secret"));
        let document: ExportDocument = serde_json::from_str(&json).unwrap();
        import_document(
            &target,
            document,
            false,
            Some(&prompter("transfer-passphrase")),
        )
        .unwrap();
        let saved = find_connection(&target, "prod").unwrap();
        assert_eq!(
            decrypt(&target, saved.secret.as_deref().unwrap()).unwrap(),
            b"secret"
        );
        assert_ne!(saved.secret.as_deref(), Some(source_secret.as_str()));
    }

    #[test]
    fn encrypted_import_rejects_wrong_passphrase_and_missing_terminal() {
        let temp = TempDir::new().unwrap();
        let source = context(&temp, "source");
        let export = export_document(
            &source,
            vec![connection("prod", Some("secret"), &source)],
            true,
            Some(&prompter("right")),
        )
        .unwrap();
        assert!(
            import_document(
                &context(&temp, "wrong"),
                export,
                false,
                Some(&prompter("wrong"))
            )
            .is_err()
        );

        let no_passwords = export_document(&source, vec![], true, None);
        assert!(no_passwords.is_err());
    }

    #[test]
    fn import_validates_document_shape_and_connection_records() {
        let temp = TempDir::new().unwrap();
        let context = context(&temp, "target");
        let mut export = ExportDocument {
            version: EXPORT_VERSION,
            count: 1,
            connections: Some(vec![PortableConnection {
                name: "bad/name".to_owned(),
                host: "127.0.0.1".to_owned(),
                port: DEFAULT_PORT,
                username: DEFAULT_USERNAME.to_owned(),
                schema: None,
                driver: DEFAULT_DRIVER.to_owned(),
                password: None,
            }]),
            encrypted_payload: None,
            salt: None,
        };
        assert!(import_document(&context, export.clone(), false, None).is_err());
        export.version += 1;
        assert!(import_document(&context, export, false, None).is_err());
    }

    #[test]
    fn import_refuses_collisions_unless_replaced_and_preserves_password() {
        let temp = TempDir::new().unwrap();
        let context = context(&temp, "target");
        upsert_connection(
            &context,
            &connection("prod", Some("local-secret"), &context),
        )
        .unwrap();
        let export = ExportDocument {
            version: EXPORT_VERSION,
            count: 1,
            connections: Some(vec![PortableConnection {
                name: "prod".to_owned(),
                host: "new-host".to_owned(),
                port: DEFAULT_PORT,
                username: DEFAULT_USERNAME.to_owned(),
                schema: None,
                driver: DEFAULT_DRIVER.to_owned(),
                password: None,
            }]),
            encrypted_payload: None,
            salt: None,
        };
        assert!(import_document(&context, export, false, None).is_err());
        let export = ExportDocument {
            version: EXPORT_VERSION,
            count: 1,
            connections: Some(vec![PortableConnection {
                name: "prod".to_owned(),
                host: "new-host".to_owned(),
                port: DEFAULT_PORT,
                username: DEFAULT_USERNAME.to_owned(),
                schema: None,
                driver: DEFAULT_DRIVER.to_owned(),
                password: None,
            }]),
            encrypted_payload: None,
            salt: None,
        };
        import_document(&context, export, true, None).unwrap();
        let saved = find_connection(&context, "prod").unwrap();
        assert_eq!(saved.host, "new-host");
        assert_eq!(
            decrypt(&context, saved.secret.as_deref().unwrap()).unwrap(),
            b"local-secret"
        );
    }

    #[test]
    fn export_file_is_private_and_never_overwritten() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("connections.json");
        write_private_file(&path, b"first").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"first");
        assert!(write_private_file(&path, b"second").is_err());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
}
