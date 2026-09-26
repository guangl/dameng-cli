mod report;
mod table;

pub(crate) use report::report;

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use dameng_cli::{Config, PluginStore, SelfUpdateOptions, self_update_with_options};
use std::ffi::OsString;

#[derive(Parser)]
#[command(name = "dm", version, about = "Plugin host for Dameng database tools")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Install a prebuilt Rust plugin from a directory or HTTPS Git repository.
    Install {
        source: String,
        /// Install an exact Git tag, branch or commit.
        #[arg(long)]
        rev: Option<String>,
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

fn print_no_plugins() {
    println!("No plugins installed. Run `dm install <source>` to add one.");
}

pub fn run(config: &Config) -> Result<i32> {
    let cli = Cli::parse();
    let store = PluginStore::from_env()?;
    match cli.command {
        Command::Install { source, rev } => {
            let manifest = store.install_with_revision(&source, rev.as_deref())?;
            println!("Installed {} {}", manifest.name, manifest.version);
        }
        Command::List { json } => {
            let plugins = store.list_info()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&plugins)?);
            } else if plugins.is_empty() {
                print_no_plugins();
            } else {
                print!("{}", table::render(&plugins));
            }
        }
        Command::Info { name, json } => {
            let plugin = store.info(&name)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&plugin)?);
            } else {
                println!("Name: {}", plugin.manifest.name);
                println!("Version: {}", plugin.manifest.version);
                println!("Source: {}", plugin.source.as_deref().unwrap_or("unknown"));
                println!(
                    "Revision: {}",
                    plugin.revision.as_deref().unwrap_or("unknown")
                );
                println!("SHA-256: {}", plugin.checksum);
                if !plugin.manifest.permissions.is_empty() {
                    println!("Permissions: {}", plugin.manifest.permissions.join(", "));
                }
                if !plugin.manifest.environment.is_empty() {
                    println!("Environment: {}", plugin.manifest.environment.join(", "));
                }
            }
        }
        Command::Update { name, all } => {
            if all {
                let results = store.update_all();
                let mut failures = Vec::new();
                let mut updated = Vec::new();
                for (name, result) in results {
                    match result {
                        Ok(manifest) => {
                            println!("Updated {name} to {}", manifest.version);
                            updated.push(name);
                        }
                        Err(error) => failures.push(format!("{name}: {error:#}")),
                    }
                }
                if !failures.is_empty() {
                    anyhow::bail!("Some updates failed:\n{}", failures.join("\n"));
                }
                if updated.is_empty() {
                    print_no_plugins();
                }
            } else {
                let name = name.context("Provide a plugin name or use --all")?;
                let manifest = store.update(&name)?;
                println!("Updated {} to {}", manifest.name, manifest.version);
            }
        }
        Command::Outdated { json } => {
            let statuses = store.outdated()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&statuses)?);
            } else if statuses.is_empty() {
                print_no_plugins();
            } else {
                for status in statuses {
                    println!(
                        "{}\t{}\t{}\t{}",
                        status.name,
                        status.installed_version,
                        status.available_version.as_deref().unwrap_or("unknown"),
                        if status.update_available {
                            "update available"
                        } else {
                            "current"
                        }
                    );
                }
            }
        }
        Command::Verify { name } => {
            let names = store.verify(name.as_deref())?;
            if names.is_empty() {
                print_no_plugins();
            } else {
                for name in names {
                    println!("Verified {name}");
                }
            }
        }
        Command::Doctor { repair, json } => {
            let report = store.doctor(repair)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else if report.issues.is_empty() {
                println!("Plugin store is healthy");
            } else {
                for issue in &report.issues {
                    println!("Issue: {issue}");
                }
                for repair in &report.repairs {
                    println!("Repaired: {repair}");
                }
                if !repair && report.repairs.is_empty() {
                    println!("Run dm doctor --repair to repair recoverable issues");
                }
            }
        }
        Command::SelfUpdate {
            check,
            version,
            force,
            target,
            json,
        } => {
            let repository = config.update_repository();
            let result = self_update_with_options(SelfUpdateOptions {
                version: version.as_deref(),
                check_only: check,
                force,
                target: target.as_deref(),
                repository: repository.as_deref(),
            })?;
            if json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else if result.updated {
                println!(
                    "Updated dm from {} to {}",
                    result.current_version, result.available_version
                );
            } else if result.current_version == result.available_version {
                println!("dm {} is current", result.current_version);
            } else {
                println!(
                    "dm {} is installed; {} is available",
                    result.current_version, result.available_version
                );
            }
        }
        Command::Completions { shell } => {
            clap_complete::generate(shell, &mut Cli::command(), "dm", &mut std::io::stdout());
        }
        Command::Uninstall { name } => {
            store.uninstall(&name)?;
            println!("Uninstalled {name}");
        }
        Command::Plugin(args) => {
            let name = args
                .first()
                .context("Missing plugin name")?
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Plugin name must be UTF-8"))?;
            return store.run(name, &args[1..]);
        }
    }
    Ok(0)
}
