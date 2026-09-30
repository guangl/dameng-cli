//! Argument definitions for the `dm` command line.

use clap::{Parser, Subcommand};
use std::ffi::OsString;

/// The `dm` command line.
#[derive(Parser)]
#[command(
    name = "dm",
    version,
    about = "Install and run plugins for Dameng database tools",
    after_help = "Getting started:\n  dm list                 Show installed plugins\n  dm info <name>          Find a plugin's settings and data\n  dm <name> --help         Explore a plugin's commands\n  dm doctor               Diagnose installation problems"
)]
pub struct Cli {
    /// Subcommand to run.
    #[command(subcommand)]
    pub command: Command,
}

/// Every subcommand the host implements.
#[derive(Subcommand)]
pub enum Command {
    /// Install a prebuilt Rust plugin from a directory or HTTPS Git repository.
    Install {
        source: String,
        /// Install an exact Git tag, branch or commit.
        #[arg(long)]
        rev: Option<String>,
        /// Replace an installed plugin of the same name instead of refusing;
        /// its config/data/cache directories are kept.
        #[arg(long)]
        replace: bool,
    },
    /// List installed plugins.
    List {
        #[arg(long)]
        json: bool,
    },
    /// Show installation metadata for one plugin.
    Info {
        name: String,
        #[arg(long)]
        json: bool,
    },
    /// Atomically update one plugin or every installed plugin.
    Update {
        name: Option<String>,
        #[arg(long, conflicts_with = "name")]
        all: bool,
    },
    /// Check installed plugins for newer versions.
    Outdated {
        #[arg(long)]
        json: bool,
    },
    /// Verify installed plugin manifests and binary checksums.
    Verify { name: Option<String> },
    /// Diagnose and optionally repair plugin-store inconsistencies.
    Doctor {
        #[arg(long)]
        repair: bool,
        #[arg(long)]
        json: bool,
    },
    /// Securely update the dm host from its GitHub Release.
    SelfUpdate {
        #[arg(long)]
        check: bool,
        #[arg(long)]
        version: Option<String>,
        /// Reinstall or downgrade even when the requested version is not newer.
        #[arg(long)]
        force: bool,
        /// Override the release target triple for this update.
        #[arg(long)]
        target: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Generate shell completion definitions on stdout.
    Completions { shell: clap_complete::Shell },
    /// Remove an installed plugin.
    Uninstall { name: String },
    /// Run an installed plugin, forwarding all remaining arguments unchanged.
    #[command(external_subcommand)]
    Plugin(Vec<OsString>),
}
