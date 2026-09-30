//! The "dm db" command line as clap derives it.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "dm db",
    about = "Manage saved Dameng database connections",
    after_help = "Getting started:\n  dm db add prod         Save a connection interactively\n  dm db list             Show saved connections\n  dm db export --file connections.json\n\nThis plugin reads its own configuration file (<config dir>/config.toml, see dm info db):\n  [defaults] port, username, driver, schema\n  [connect] timeout, probe"
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: DbCommand,
}

#[derive(Subcommand)]
pub(crate) enum DbCommand {
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
    List {
        /// Print the same fields as machine-readable JSON instead of a table.
        #[arg(long)]
        json: bool,
    },
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
