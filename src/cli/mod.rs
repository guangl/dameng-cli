//! Command-line interface for the `dm` plugin host.
//!
//! The tree is split by responsibility: `args` holds the clap definitions,
//! the sibling modules implement one group of subcommands each, and
//! `report`/`table` render the human-facing output.

mod args;
mod completions;
mod doctor;
mod plugins;
pub mod report;
mod self_update;
mod settings;
pub mod table;
mod update;

pub use args::{Cli, Command};
pub use report::report;

use crate::{Config, PluginStore};
use anyhow::Result;
use clap::Parser;

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
            release_source,
            release_tag,
            release_target,
            rev,
            replace,
        } => {
            if let Some(repository) = release_source {
                let manifest = store.install_release_package(
                    std::path::Path::new(&source),
                    &repository,
                    release_tag.as_deref().unwrap_or_default(),
                    release_target.as_deref(),
                    replace,
                )?;
                println!("已安装 {} {}", manifest.name, manifest.version);
            } else {
                plugins::install(&store, &source, rev.as_deref(), replace)?;
            }
        }
        Command::List { json } => plugins::list(&store, json)?,
        Command::Info { name, json } => plugins::info(&store, &name, json)?,
        Command::Update { name, all, json } => update::run(&store, name.as_deref(), all, json)?,
        Command::Doctor { name, repair, json } => {
            if let Some(name) = name {
                anyhow::ensure!(!repair, "插件环境检查不支持 --repair");
                let mut args = vec![std::ffi::OsString::from("doctor")];
                if json {
                    args.push("--json".into());
                }
                return store.run(&name, &args);
            }
            doctor::doctor(&store, repair, json)?;
        }
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
        Command::Completions { shell } => print!("{}", completions::script(shell)),
        Command::Complete {
            mut words,
            empty_word,
        } => {
            if words.first().is_some_and(|word| word == "--") {
                words.remove(0);
            }
            if empty_word {
                words.push(String::new());
            }
            completions::complete(&store, &words)?;
        }
        Command::Config { command } => settings::run(config, command)?,
        Command::Uninstall { name, purge, yes } => plugins::uninstall(&store, &name, purge, yes)?,
        Command::Plugin(args) => return plugins::run_plugin(&store, &args),
    }
    Ok(0)
}

/// Tell the user that no plugin is installed yet.
pub(crate) fn print_no_plugins() {
    println!("尚无插件。运行 `dm install <包目录或仓库地址>` 安装；官方安装脚本会安装 db 和 ssh。");
}
