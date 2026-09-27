//! Manage saved SSH server connections.
//!
//! The crate root only declares the modules below and re-exports their public
//! items, so every existing `dm_plugin_ssh::{...}` path keeps resolving.

use dm_plugin_sdk::{Context as PluginContext, Plugin, PluginResult};

mod auth;
mod commands;
mod config;
mod crypto;
mod export;
mod hints;
mod import;
mod list;
mod private_file;
mod prompts;
mod servers;
mod ssh_command;

pub use auth::resolve_auth;
pub use commands::{run_cli, run_with_prompter};
pub use config::{SshConfig, SshDefaults, SshTestSettings, config_path, load_config};
pub use crypto::{decrypt, encrypt, hex, key_path, machine_key, unhex};
// The export tests reach these internals through the public API; they stay
// doc(hidden) so the documented surface does not grow.
#[doc(hidden)]
pub use export::{EXPORT_VERSION, ExportDocument, PortableServer, export_document};
pub use hints::ssh_hint;
#[doc(hidden)]
pub use import::import_document;
pub use list::{ServerSummary, render_json, render_table};
#[doc(hidden)]
pub use private_file::{write_private_file, write_private_file_with};
pub use prompts::{
    Prompter, TerminalPrompter, resolve_passphrase, resolve_password, resolve_port,
    resolve_required,
};
pub use servers::{
    Server, database_path, load_servers, open_database, remove_server, upsert_server, validate_name,
};
pub use ssh_command::ssh_command;

pub struct SshPlugin;

impl Plugin for SshPlugin {
    fn run(&self, context: PluginContext) -> PluginResult {
        match run_cli(&context) {
            Ok(code) => Ok(code),
            Err(error) => {
                eprintln!("dm ssh: {error:#}");
                eprintln!("提示：{}", ssh_hint(&error));
                Ok(1)
            }
        }
    }
}
