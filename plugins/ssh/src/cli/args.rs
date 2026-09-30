//! The "dm ssh" command line as clap derives it.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "dm ssh",
    about = "Manage saved SSH server connections",
    after_help = "Getting started:\n  dm ssh add prod         Save a connection interactively\n  dm ssh list             Show saved connections\n  dm ssh export --file connections.json\n\nThis plugin reads its own configuration file (<config dir>/config.toml, see `dm info ssh`):\n  [defaults] port, username, auth, key\n  [test] connect_timeout"
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: SshCommand,
}

#[derive(Subcommand)]
pub(crate) enum SshCommand {
    /// Add or replace a saved SSH server; prompts interactively for any value that is omitted.
    Add {
        /// SSH server name (prompted when omitted on a terminal).
        name: Option<String>,
        /// SSH host (prompted when omitted on a terminal).
        #[arg(long)]
        host: Option<String>,
        /// SSH port, default 22 (prompted when omitted on a terminal).
        #[arg(long)]
        port: Option<u16>,
        /// SSH username (prompted when omitted on a terminal).
        #[arg(long)]
        username: Option<String>,
        /// Password for password authentication; prompted when omitted on a terminal.
        #[arg(long)]
        password: Option<String>,
        /// Private key path for key authentication; prompted when key authentication is chosen.
        #[arg(long)]
        key: Option<PathBuf>,
        /// Key passphrase; prompted when omitted on a terminal.
        #[arg(long)]
        passphrase: Option<String>,
    },
    /// List saved SSH servers.
    List {
        /// Print the same fields as machine-readable JSON instead of a table.
        #[arg(long)]
        json: bool,
    },
    /// Remove a saved SSH server.
    Remove { name: String },
    /// Export server settings. Passwords and key passphrases are omitted unless encrypted export is requested.
    Export {
        /// Write the JSON export to this file instead of stdout; it never overwrites.
        #[arg(long)]
        file: Option<PathBuf>,
        /// Include passwords and key passphrases encrypted with an export passphrase.
        #[arg(long)]
        include_secrets: bool,
    },
    /// Import server settings from a JSON export.
    Import {
        file: PathBuf,
        /// Replace servers with matching names.
        #[arg(long)]
        replace: bool,
    },
    /// Test an SSH connection non-interactively.
    Test { name: String },
    /// Start an interactive SSH session.
    Ssh { name: String },
}
