//! Parsing and dispatch of the "dm ssh" subcommands.

mod add;
mod args;
mod transfer;

use crate::cli::add::{AddRequest, add_server};
use crate::cli::args::{Cli, SshCommand};
use crate::domain::ssh_command::ssh_command;
use crate::storage::servers::{load_servers, remove_server};
use crate::ui::list::{render_json, render_table};
use crate::ui::prompts::{Prompter, terminal_prompter};
use anyhow::{Context, Result, ensure};
use clap::Parser;
use dm_plugin_sdk::Context as PluginContext;
use std::ffi::OsString;

pub fn run_cli(context: &PluginContext) -> Result<i32> {
    run_with_prompter(context, terminal_prompter())
}

/// Command dispatch with an injectable prompt source, so tests cover the
/// interactive branches without a terminal.
pub fn run_with_prompter(context: &PluginContext, prompter: Option<&dyn Prompter>) -> Result<i32> {
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
            let name = add_server(
                context,
                &AddRequest {
                    name,
                    host,
                    port,
                    username,
                    password,
                    key,
                    passphrase,
                },
                prompter,
            )?;
            println!("Saved SSH server {name}");
        }
        SshCommand::List { json } => {
            let servers = load_servers(context)?;
            if json {
                println!("{}", render_json(&servers)?);
            } else {
                let table = render_table(&servers);
                if table.is_empty() {
                    println!("No saved SSH servers. Run `dm ssh add <name>` to add one.");
                } else {
                    println!("{table}");
                }
            }
        }
        SshCommand::Remove { name } => {
            remove_server(context, &name)?;
            println!("Removed SSH server {name}");
        }
        SshCommand::Export {
            file,
            include_secrets,
        } => transfer::export(context, file, include_secrets, prompter)?,
        SshCommand::Import { file, replace } => transfer::import(context, file, replace, prompter)?,
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
