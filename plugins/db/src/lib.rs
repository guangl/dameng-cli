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
//! placeholder that reports it is not implemented; see src/domain/driver.rs. Run
//! "dm info db" to see where the configuration file belongs.

mod cli;
mod domain;
mod storage;
mod transfer;
mod ui;
use dm_plugin_support::private_file;

use dm_plugin_sdk::{Context as PluginContext, Plugin, PluginResult};

pub use cli::{run_cli, run_with, run_with_prompter};
pub use domain::connection_string::build_connection_string;
pub use domain::driver;
pub use domain::sql::{format_result, read_sql, write_result};
pub use domain::types::{
    ConnectionSpec, DEFAULT_DRIVER, DEFAULT_LOGIN_TIMEOUT, DEFAULT_PORT, DEFAULT_PROBE,
    DEFAULT_USERNAME, Database, DatabaseConnection, DatabaseFactory, Outcome, QueryResult, Session,
};
#[doc(hidden)]
pub use private_file::{write_private_file, write_private_file_with};
pub use storage::config::{DbConfig, DbConnect, DbDefaults, config_path, load_config};
pub use storage::connections::{
    connection_password, database_path, find_connection, load_connections, open_database,
    remove_connection, upsert_connection, validate_name,
};
pub use storage::crypto::{decrypt, encrypt, hex, key_path, machine_key, unhex};
#[doc(hidden)]
pub use transfer::export::{EXPORT_VERSION, ExportDocument, PortableConnection, export_document};
#[doc(hidden)]
pub use transfer::import::import_document;
pub use ui::hints::db_hint;
pub use ui::list::{ConnectionSummary, render_json, render_table};
pub use ui::prompts::{
    Prompter, TerminalPrompter, resolve_optional, resolve_password, resolve_port, resolve_required,
    resolve_with_default,
};

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
