use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use dameng_cli::{PluginStore, cleanup_self_update_backup, scaffold_plugin, self_update};
use std::{ffi::OsString, path::PathBuf};

#[derive(Parser)]
#[command(name = "dm", version, about = "Plugin host for Dameng database tools")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create a new Rust plugin project.
    New {
        name: String,
        #[arg(long)]
        directory: Option<PathBuf>,
    },
    /// Build and install a Rust plugin from a directory or HTTPS Git repository.
    Install {
        source: String,
        /// Install an exact Git tag, branch or commit.
        #[arg(long)]
        rev: Option<String>,
        /// Accept newly declared permissions and environment variables.
        #[arg(long)]
        accept_permissions: bool,
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
    /// Search configured plugin sources.
    Search {
        #[arg(default_value = "")]
        query: String,
        /// Search a remote HTTPS registry index instead of the local registry.
        #[arg(long)]
        remote: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Atomically update one plugin or every installed plugin.
    Update {
        name: Option<String>,
        #[arg(long, conflicts_with = "name")]
        all: bool,
        /// Accept newly declared permissions and environment variables.
        #[arg(long)]
        accept_permissions: bool,
    },
    /// Check installed plugins for newer versions.
    Outdated {
        #[arg(long)]
        json: bool,
    },
    /// Enable an installed plugin.
    Enable { name: String },
    /// Disable an installed plugin without removing it.
    Disable { name: String },
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
        #[arg(long)]
        json: bool,
    },
    /// Generate shell completion definitions on stdout.
    Completions { shell: clap_complete::Shell },
    /// Remove an installed plugin.
    Uninstall { name: String },
    /// Manage named plugin sources stored in SQLite.
    Registry {
        #[command(subcommand)]
        command: RegistryCommand,
    },
    /// Run an installed plugin, forwarding all remaining arguments unchanged.
    #[command(external_subcommand)]
    Plugin(Vec<OsString>),
}

#[derive(Subcommand)]
enum RegistryCommand {
    /// Add or replace a name-to-HTTPS-Git mapping.
    Add { name: String, source: String },
    /// Fetch and merge a remote JSON registry index.
    Sync { url: String },
    /// List configured plugin sources.
    List,
    /// Remove a configured plugin source.
    Remove { name: String },
}

fn run() -> Result<i32> {
    let cli = Cli::parse();
    let store = PluginStore::from_env()?;
    match cli.command {
        Command::New { name, directory } => {
            let destination =
                directory.unwrap_or_else(|| PathBuf::from(format!("dm-plugin-{name}")));
            scaffold_plugin(&name, &destination)?;
            println!("Created {}", destination.display());
            println!("Run cargo generate-lockfile in the new directory before installing");
        }
        Command::Install {
            source,
            rev,
            accept_permissions,
        } => {
            let manifest =
                store.install_with_consent(&source, rev.as_deref(), accept_permissions)?;
            println!("Installed {} {}", manifest.name, manifest.version);
        }
        Command::List { json } => {
            let plugins = store.list_info()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&plugins)?);
            } else {
                for plugin in plugins {
                    println!(
                        "{}\t{}\t{}\t{}",
                        plugin.manifest.name,
                        plugin.manifest.version,
                        if plugin.enabled {
                            "enabled"
                        } else {
                            "disabled"
                        },
                        plugin.manifest.description
                    );
                }
            }
        }
        Command::Info { name, json } => {
            let plugin = store.info(&name)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&plugin)?);
            } else {
                println!("Name: {}", plugin.manifest.name);
                println!("Version: {}", plugin.manifest.version);
                println!(
                    "Status: {}",
                    if plugin.enabled {
                        "enabled"
                    } else {
                        "disabled"
                    }
                );
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
        Command::Search {
            query,
            remote,
            json,
        } => {
            let results = if let Some(url) = remote {
                store.search_remote(&url, &query)?
            } else {
                store.search(&query)?
            };
            if json {
                println!("{}", serde_json::to_string_pretty(&results)?);
            } else {
                for (name, source) in results {
                    println!("{name}\t{source}");
                }
            }
        }
        Command::Update {
            name,
            all,
            accept_permissions,
        } => {
            if all {
                let results = store.update_all_with_consent(accept_permissions);
                let mut failures = Vec::new();
                for (name, result) in results {
                    match result {
                        Ok(manifest) => println!("Updated {name} to {}", manifest.version),
                        Err(error) => failures.push(format!("{name}: {error:#}")),
                    }
                }
                if !failures.is_empty() {
                    anyhow::bail!("Some updates failed:\n{}", failures.join("\n"));
                }
            } else {
                let name = name.context("Provide a plugin name or use --all")?;
                let manifest = store.update_with_consent(&name, accept_permissions)?;
                println!("Updated {} to {}", manifest.name, manifest.version);
            }
        }
        Command::Outdated { json } => {
            let statuses = store.outdated()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&statuses)?);
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
        Command::Enable { name } => {
            store.set_enabled(&name, true)?;
            println!("Enabled {name}");
        }
        Command::Disable { name } => {
            store.set_enabled(&name, false)?;
            println!("Disabled {name}");
        }
        Command::Verify { name } => {
            for name in store.verify(name.as_deref())? {
                println!("Verified {name}");
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
            json,
        } => {
            let result = self_update(version.as_deref(), check)?;
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
        Command::Registry { command } => match command {
            RegistryCommand::Add { name, source } => {
                store.registry_add(&name, &source)?;
                println!("Registered {name}");
            }
            RegistryCommand::Sync { url } => {
                let count = store.registry_sync(&url)?;
                println!("Synced {count} plugin sources");
            }
            RegistryCommand::List => {
                for (name, source) in store.registry_list()? {
                    println!("{name}\t{source}");
                }
            }
            RegistryCommand::Remove { name } => {
                store.registry_remove(&name)?;
                println!("Removed {name}");
            }
        },
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

fn main() {
    let _ = cleanup_self_update_backup();
    let code = match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("dm: {error:#}");
            1
        }
    };
    std::process::exit(code);
}
