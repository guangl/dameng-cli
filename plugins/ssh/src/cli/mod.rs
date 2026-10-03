//! Parsing and dispatch of the "dm ssh" subcommands.

mod add;
mod args;
mod completion;
mod edit;
mod fields;
mod session;
mod settings;
mod transfer;

use crate::cli::add::{AddRequest, add_server};
use crate::cli::args::{Cli, SshCommand};
use crate::storage::servers::{load_servers, remove_server};
use crate::ui::list::{render_json, render_table};
use crate::ui::prompts::{Prompter, terminal_prompter};
use anyhow::Result;
use clap::Parser;
use dm_plugin_sdk::Context as PluginContext;
use std::ffi::OsString;

pub fn run_cli(context: &PluginContext) -> Result<i32> {
    run_with_prompter(context, terminal_prompter())
}

/// Command dispatch with an injectable prompt source, so tests cover the
/// interactive branches without a terminal.
pub fn run_with_prompter(context: &PluginContext, prompter: Option<&dyn Prompter>) -> Result<i32> {
    run_with_validator(context, prompter, &crate::test_server)
}

/// Injectable transport validation for library integrations and isolated tests.
/// The executable always uses the native SSH validator.
#[doc(hidden)]
pub fn run_with_validator(
    context: &PluginContext,
    prompter: Option<&dyn Prompter>,
    validator: &dyn Fn(&PluginContext, &crate::Server) -> Result<()>,
) -> Result<i32> {
    if completion::handle(context)? {
        return Ok(0);
    }
    let mut argv = vec![OsString::from("dm ssh")];
    argv.extend(context.args.iter().cloned());
    let cli = Cli::parse_from(argv);
    match cli.command {
        SshCommand::Add {
            name,
            fields,
            replace,
        } => {
            let name = add_server(
                context,
                &AddRequest {
                    name,
                    host: fields.host,
                    port: fields.port,
                    username: fields.username,
                    password: fields.password,
                    key: fields.key,
                    passphrase: fields.passphrase,
                },
                prompter,
                replace,
                fields.yes,
                validator,
            )?;
            println!("SSH 连通性与认证测试成功，已保存 SSH 连接 {name}");
        }
        SshCommand::Edit { name, fields } => edit::edit(context, &name, fields, prompter)?,
        SshCommand::Doctor { json } => return settings::doctor(context, json),
        SshCommand::Config { command } => settings::config(context, command)?,
        SshCommand::List { json } => {
            let servers = load_servers(context)?;
            if json {
                println!("{}", render_json(&servers)?);
            } else {
                let table = render_table(&servers);
                if table.is_empty() {
                    println!("尚无 SSH 连接。运行 `dm ssh add <name>` 添加连接。");
                } else {
                    println!("{table}");
                }
            }
        }
        SshCommand::Remove { name, yes } => {
            anyhow::ensure!(
                load_servers(context)?
                    .iter()
                    .any(|entry| entry.name == name),
                "SSH server '{name}' is not configured"
            );
            dm_plugin_support::interaction::confirm(prompter, yes, &format!("删除连接 {name}？"))?;
            remove_server(context, &name)?;
            println!("已删除 SSH 连接 {name}");
        }
        SshCommand::Export {
            file,
            include_secrets,
        } => transfer::export(context, file, include_secrets, prompter)?,
        SshCommand::Import { file, replace } => transfer::import(context, file, replace, prompter)?,
        SshCommand::Test { name } => {
            let name = dm_plugin_support::interaction::select_name(
                name,
                &load_servers(context)?
                    .into_iter()
                    .map(|entry| entry.name)
                    .collect::<Vec<_>>(),
                prompter,
            )?;
            session::test(context, &name)?;
        }
        SshCommand::Ssh { name } => {
            let name = dm_plugin_support::interaction::select_name(
                name,
                &load_servers(context)?
                    .into_iter()
                    .map(|entry| entry.name)
                    .collect::<Vec<_>>(),
                prompter,
            )?;
            return session::connect(context, &name);
        }
    }
    Ok(0)
}
