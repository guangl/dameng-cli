//! How "dm db list" reports saved connections: a table for people, JSON for scripts.

use crate::types::DatabaseConnection;
use anyhow::Result;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{ContentArrangement, Table};
use serde::Serialize;

/// Width used when stdout is not a terminal, matching the host's "dm list".
const NON_TTY_WIDTH: u16 = 120;
/// Placeholder shown in the table when a connection selects no schema.
const NO_SCHEMA: &str = "-";

/// One saved connection as "dm db list --json" reports it.
///
/// The password never appears here: summaries are built from the stored row
/// itself, so no code path can leak the encrypted secret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConnectionSummary {
    /// Connection name used by every other "dm db" command.
    pub name: String,
    /// Database host.
    pub host: String,
    /// Database port.
    pub port: u16,
    /// Database user.
    pub username: String,
    /// Schema selected after connecting, if any.
    pub schema: Option<String>,
    /// Driver name recorded for the connection.
    pub driver: String,
}

/// Summarise saved connections for "dm db list --json".
pub(crate) fn summaries(connections: &[DatabaseConnection]) -> Vec<ConnectionSummary> {
    connections
        .iter()
        .map(|connection| ConnectionSummary {
            name: connection.name.clone(),
            host: connection.host.clone(),
            port: connection.port,
            username: connection.username.clone(),
            schema: connection.schema.clone(),
            driver: connection.driver.clone(),
        })
        .collect()
}

/// Render "dm db list --json"; an empty store prints "[]".
pub fn render_json(connections: &[DatabaseConnection]) -> Result<String> {
    Ok(serde_json::to_string_pretty(&summaries(connections))?)
}

/// Render saved connections as a bordered UTF-8 table.
///
/// An empty store renders an empty string, so "dm db list" stays silent instead
/// of printing a header with no rows.
pub fn render_table(connections: &[DatabaseConnection]) -> String {
    if connections.is_empty() {
        return String::new();
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_truncation_indicator("…")
        .set_header(["Name", "Host", "Port", "User", "Schema", "Driver"]);

    // comfy-table auto-detects the terminal width only when stdout is a TTY.
    // Keep piped output deterministic and reasonably narrow as well.
    if !table.is_tty() {
        table.set_width(NON_TTY_WIDTH);
    }

    for connection in connections {
        table.add_row([
            connection.name.as_str(),
            connection.host.as_str(),
            &connection.port.to_string(),
            connection.username.as_str(),
            connection.schema.as_deref().unwrap_or(NO_SCHEMA),
            connection.driver.as_str(),
        ]);
    }

    table.to_string()
}
