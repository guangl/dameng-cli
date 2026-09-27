//! Parsing and dispatch of the "dm ssh" subcommands.

mod add;
mod cli;

use crate::commands::add::{AddRequest, add_server};
use crate::commands::cli::{Cli, SshCommand};
use crate::export::{ExportDocument, export_document};
use crate::import::import_document;
use crate::list::{render_json, render_table};
use crate::private_file::write_private_file;
use crate::prompts::{Prompter, terminal_prompter};
use crate::servers::{load_servers, remove_server};
use crate::ssh_command::ssh_command;
use anyhow::{Context, Result, ensure};
use clap::Parser;
use dm_plugin_sdk::Context as PluginContext;
use std::{ffi::OsString, fs, io::Write};

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
                // An empty store renders nothing at all, so "--json" is the
                // only form that reports an empty list explicitly.
                if !table.is_empty() {
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
        } => {
            let servers = load_servers(context)?;
            let document = export_document(context, servers, include_secrets, prompter)?;
            let json = serde_json::to_vec_pretty(&document)?;
            match file {
                Some(path) => {
                    write_private_file(&path, &json)?;
                    println!(
                        "Exported {} SSH servers to {}",
                        document.count,
                        path.display()
                    );
                }
                None => {
                    std::io::stdout().write_all(&json)?;
                    println!();
                }
            }
        }
        SshCommand::Import { file, replace } => {
            let document: ExportDocument = serde_json::from_slice(
                &fs::read(&file).with_context(|| format!("Read {}", file.display()))?,
            )
            .with_context(|| format!("Parse SSH server export {}", file.display()))?;
            let servers = import_document(context, document, replace, prompter)?;
            println!("Imported {servers} SSH servers");
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
