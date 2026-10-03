//! Manage saved SSH server connections.
//!
//! The crate root only declares the modules below and re-exports their public
//! items, so every existing `dm_plugin_ssh::{...}` path keeps resolving.

use dm_plugin_sdk::{Context as PluginContext, Plugin, PluginResult};

mod cli;
mod domain;
mod storage;
mod transfer;
mod ui;
use dm_plugin_support::private_file;

pub use cli::{run_cli, run_with_prompter, run_with_validator};
pub use domain::auth::resolve_auth;
pub use domain::ssh_command::{server_by_name, test_server};
#[doc(hidden)]
pub use private_file::{write_private_file, write_private_file_with};
pub use storage::config::{SshConfig, SshDefaults, SshTestSettings, config_path, load_config};
pub use storage::crypto::{decrypt, encrypt, hex, key_path, machine_key, unhex};
pub use storage::servers::{
    Server, database_path, load_servers, open_database, remove_server, upsert_server, validate_name,
};
#[doc(hidden)]
pub use transfer::export::{EXPORT_VERSION, ExportDocument, PortableServer, export_document};
#[doc(hidden)]
pub use transfer::import::import_document;
pub use ui::hints::ssh_hint;
pub use ui::list::{ServerSummary, render_json, render_table};
pub use ui::prompts::{
    Prompter, TerminalPrompter, resolve_passphrase, resolve_password, resolve_port,
    resolve_required,
};

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
