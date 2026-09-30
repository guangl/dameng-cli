//! How "dm ssh list" reports servers: a table for people, JSON for scripts.

use crate::storage::servers::Server;
use anyhow::Result;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{ContentArrangement, Table};
use serde::Serialize;

/// Width used when stdout is not a terminal, matching the host's "dm list".
const NON_TTY_WIDTH: u16 = 120;
/// Placeholder shown for the columns a server does not use.
const NOT_SET: &str = "-";

/// One saved server as "dm ssh list --json" reports it.
///
/// The password and key passphrase never appear here: summaries are built from
/// the stored row itself, so no code path can leak the encrypted secret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ServerSummary {
    /// Server name used by every other "dm ssh" command.
    pub name: String,
    /// SSH host.
    pub host: String,
    /// SSH port.
    pub port: u16,
    /// SSH user.
    pub username: String,
    /// "password" or "key".
    pub auth_type: String,
    /// Private key path; only set for key authentication.
    pub key_path: Option<String>,
}

/// Summarise saved servers for "dm ssh list --json".
pub fn summaries(servers: &[Server]) -> Vec<ServerSummary> {
    servers
        .iter()
        .map(|server| ServerSummary {
            name: server.name.clone(),
            host: server.host.clone(),
            port: server.port,
            username: server.username.clone(),
            auth_type: server.auth_type.clone(),
            key_path: server.key_path.clone(),
        })
        .collect()
}

/// Render "dm ssh list --json"; an empty store prints "[]".
pub fn render_json(servers: &[Server]) -> Result<String> {
    Ok(serde_json::to_string_pretty(&summaries(servers))?)
}

/// Render saved servers as a bordered UTF-8 table.
///
/// An empty store renders an empty string, so "dm ssh list" stays silent
/// instead of printing a header with no rows.
pub fn render_table(servers: &[Server]) -> String {
    if servers.is_empty() {
        return String::new();
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_truncation_indicator("…")
        .set_header(["名称", "地址", "端口", "用户名", "认证方式", "私钥"]);

    // comfy-table auto-detects the terminal width only when stdout is a TTY.
    // Keep piped output deterministic and reasonably narrow as well.
    if !table.is_tty() {
        table.set_width(NON_TTY_WIDTH);
    }

    for server in servers {
        table.add_row([
            server.name.as_str(),
            server.host.as_str(),
            &server.port.to_string(),
            server.username.as_str(),
            server.auth_type.as_str(),
            server.key_path.as_deref().unwrap_or(NOT_SET),
        ]);
    }

    table.to_string()
}
