//! Command-line interface for the `dm` plugin host.
//!
//! The tree is split by responsibility: `args` holds the clap definitions,
//! the sibling modules implement one group of subcommands each, and
//! `report`/`table` render the human-facing output.

mod args;
mod doctor;
mod plugins;
pub mod report;
mod self_update;
pub mod table;
mod update;

pub use args::{Cli, Command};
pub use report::report;

use crate::{Config, PluginStore};
use anyhow::Result;
use clap::{CommandFactory, Parser};

/// Parse the process arguments and run the requested subcommand.
///
/// Returns the exit code the process should use. Every host subcommand reports
/// success with `0`; the plugin passthrough returns the plugin's own code.
pub fn run(config: &Config) -> Result<i32> {
    let cli = Cli::parse();
    let store = PluginStore::from_env()?
        .with_progress(config.progress()?)
        .with_plugin_environment(config.plugin_environment()?);
    match cli.command {
        Command::Install {
            source,
            rev,
            replace,
        } => plugins::install(&store, &source, rev.as_deref(), replace)?,
        Command::List { json } => plugins::list(&store, json)?,
        Command::Info { name, json } => plugins::info(&store, &name, json)?,
        Command::Update { name, all } => update::update(&store, name.as_deref(), all)?,
        Command::Outdated { json } => update::outdated(&store, json)?,
        Command::Verify { name } => plugins::verify(&store, name.as_deref())?,
        Command::Doctor { repair, json } => doctor::doctor(&store, repair, json)?,
        Command::SelfUpdate {
            check,
            version,
            force,
            target,
            json,
        } => self_update::run(
            config,
            version.as_deref(),
            check,
            force,
            target.as_deref(),
            json,
        )?,
        Command::Completions { shell } => {
            clap_complete::generate(shell, &mut Cli::command(), "dm", &mut std::io::stdout());
        }
        Command::Uninstall { name } => plugins::uninstall(&store, &name)?,
        Command::Plugin(args) => return plugins::run_plugin(&store, &args),
    }
    Ok(0)
}

/// Tell the user that no plugin is installed yet.
pub(crate) fn print_no_plugins() {
    println!("No plugins installed. Run `dm install <source>` to add one.");
}
