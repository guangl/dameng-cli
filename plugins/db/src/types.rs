//! Core value types and the driver interfaces the commands talk to.

use anyhow::Result;
use std::time::Duration;

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
