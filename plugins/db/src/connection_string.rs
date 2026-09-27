//! The ODBC connection string used by "dm db test" and "dm db exec".

use crate::config::DbConfig;
use crate::types::{ConnectionSpec, DEFAULT_LOGIN_TIMEOUT, DatabaseConnection};
use anyhow::{Result, ensure};
use std::time::Duration;

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

pub(crate) fn connection_spec(
    connection: &DatabaseConnection,
    password: &str,
    config: &DbConfig,
) -> Result<ConnectionSpec> {
    Ok(ConnectionSpec {
        connection_string: build_connection_string(connection, password)?,
        login_timeout: Duration::from_secs(config.connect.timeout.unwrap_or(DEFAULT_LOGIN_TIMEOUT)),
    })
}
