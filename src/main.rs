use anyhow::Result;
use clap::{Parser, Subcommand};
use dameng_cli::PluginStore;
use std::ffi::OsString;

#[derive(Parser)]
#[command(name = "dm", version, about = "Plugin host for Dameng database tools")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Build and install a Rust plugin from a directory or HTTPS Git repository.
    Install { source: String },
    /// List installed plugins.
    List,
    /// Remove an installed plugin.
    Uninstall { name: String },
    /// Run an installed plugin, forwarding all remaining arguments unchanged.
    #[command(external_subcommand)]
    Plugin(Vec<OsString>),
}

fn run() -> Result<i32> {
    let cli = Cli::parse();
    let store = PluginStore::from_env()?;
    match cli.command {
        Command::Install { source } => {
            let manifest = store.install(&source)?;
            println!("Installed {} {}", manifest.name, manifest.version);
        }
        Command::List => {
            for plugin in store.list()? {
                println!(
                    "{}\t{}\t{}",
                    plugin.name, plugin.version, plugin.description
                );
            }
        }
        Command::Uninstall { name } => {
            store.uninstall(&name)?;
            println!("Uninstalled {name}");
        }
        Command::Plugin(args) => {
            let name = args[0]
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Plugin name must be UTF-8"))?;
            return store.run(name, &args[1..]);
        }
    }
    Ok(0)
}

fn main() {
    let code = match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("dm: {error:#}");
            1
        }
    };
    std::process::exit(code);
}
