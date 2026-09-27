use anyhow::{Context, Result, ensure};
use clap::{Parser, Subcommand};
use dm_plugin_sdk::Context as PluginContext;
use std::{ffi::OsString, path::PathBuf};

use crate::auth::resolve_auth;
use crate::config::load_config;
use crate::prompts::{resolve_port, resolve_required, terminal_prompter};
use crate::servers::{Server, load_servers, remove_server, upsert_server, validate_name};
use crate::ssh_command::ssh_command;

#[derive(Parser)]
#[command(
    name = "dm ssh",
    about = "Manage saved SSH server connections",
    after_help = "This plugin reads its own configuration file (<config dir>/config.toml, see `dm info ssh`):\n  [defaults] port, username, auth, key\n  [test] connect_timeout"
)]
struct Cli {
    #[command(subcommand)]
    command: SshCommand,
}

#[derive(Subcommand)]
enum SshCommand {
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
    List,
    /// Remove a saved SSH server.
    Remove { name: String },
    /// Test an SSH connection non-interactively.
    Test { name: String },
    /// Start an interactive SSH session.
    Ssh { name: String },
}

pub fn run_cli(context: &PluginContext) -> Result<i32> {
    let mut argv = vec![OsString::from("dm ssh")];
    argv.extend(context.args.iter().cloned());
    let cli = Cli::parse_from(argv);
    match cli.command {
        SshCommand::Add {
            name,
            host,
            port,
            username,
            password,
            key,
            passphrase,
        } => {
            let prompter = terminal_prompter();
            // Values omitted on the command line fall back to the plugin's own
            // configuration file before any prompt.
            let config = load_config(context)?;
            let name = resolve_required(name, "Name: ", "SSH server name is required", prompter)?;
            validate_name(&name)?;
            let host = resolve_required(host, "Host: ", "SSH host is required", prompter)?;
            ensure!(!host.is_empty(), "SSH host must not be empty");
            let port = resolve_port(port.or(config.defaults.port), prompter)?;
            let username = resolve_required(
                username.or(config.defaults.username.clone()),
                "Username: ",
                "SSH username is required",
                prompter,
            )?;
            ensure!(!username.is_empty(), "SSH username must not be empty");
            let key = key.or_else(|| config.defaults.key.clone().map(PathBuf::from));
            let default_method = match config.defaults.auth.as_deref() {
                Some("key") => Some("key"),
                Some(_) => Some("password"),
                None => None,
            };
            let (auth_type, key_path, secret) =
                resolve_auth(context, password, key, passphrase, prompter, default_method)?;
            upsert_server(
                context,
                &Server {
                    name: name.clone(),
                    host,
                    port,
                    username,
                    auth_type,
                    key_path,
                    secret,
                },
            )?;
            println!("Saved SSH server {name}");
        }
        SshCommand::List => {
            for server in load_servers(context)? {
                println!(
                    "{}\t{}@{}:{}\t{}\t{}",
                    server.name,
                    server.username,
                    server.host,
                    server.port,
                    server.auth_type,
                    server.key_path.as_deref().unwrap_or("")
                );
            }
        }
        SshCommand::Remove { name } => {
            remove_server(context, &name)?;
            println!("Removed SSH server {name}");
        }
        SshCommand::Test { name } => {
            let mut command = ssh_command(context, &name, true)?;
            let status = command
                .status()
                .context("Test SSH server; password authentication requires sshpass")?;
            ensure!(status.success(), "SSH connection failed for '{name}'");
            println!("SSH server {name} is reachable");
        }
        SshCommand::Ssh { name } => {
            let mut command = ssh_command(context, &name, false)?;
            let status = command
                .status()
                .context("Start SSH session; password authentication requires sshpass")?;
            let code = match status.code() {
                Some(code) => code,
                None => {
                    #[cfg(unix)]
                    {
                        use std::os::unix::process::ExitStatusExt;
                        status.signal().map(|signal| 128 + signal).unwrap_or(0)
                    }
                    #[cfg(not(unix))]
                    {
                        anyhow::bail!("SSH session terminated without an exit code")
                    }
                }
            };
            return Ok(code);
        }
    }
    Ok(0)
}
