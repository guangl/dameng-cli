//! Parsing and dispatch of the "dm db" subcommands.

mod add;
mod args;
mod completion;
mod edit;
mod fields;
mod query;
mod settings;
mod transfer;

use crate::cli::add::{AddRequest, add_connection};
use crate::cli::args::{Cli, DbCommand};
use crate::domain::driver::PendingFactory;
use crate::domain::types::DatabaseFactory;
use crate::storage::connections::{load_connections, remove_connection};
use crate::ui::list::{render_json, render_table};
use crate::ui::prompts::{Prompter, terminal_prompter};
use anyhow::Result;
use clap::Parser;
use dm_plugin_sdk::Context as PluginContext;
use std::ffi::OsString;

pub fn run_cli(context: &PluginContext) -> Result<i32> {
    run_with_prompter(context, &PendingFactory, terminal_prompter())
}

/// Command dispatch without prompts, used by tests and scripts.
pub fn run_with(context: &PluginContext, factory: &dyn DatabaseFactory) -> Result<i32> {
    run_with_prompter(context, factory, None)
}

/// Command dispatch with an injectable driver and prompt source, so tests cover
/// every branch without a terminal or a database.
pub fn run_with_prompter(
    context: &PluginContext,
    factory: &dyn DatabaseFactory,
    prompter: Option<&dyn Prompter>,
) -> Result<i32> {
    if completion::handle(context)? {
        return Ok(0);
    }
    let mut argv = vec![OsString::from("dm db")];
    argv.extend(context.args.iter().cloned());
    let cli = Cli::parse_from(argv);
    match cli.command {
        DbCommand::Add {
            name,
            fields,
            replace,
        } => {
            anyhow::ensure!(!fields.clear_schema, "--clear-schema 仅用于 edit");
            let name = add_connection(
                context,
                &AddRequest {
                    name,
                    host: fields.host,
                    port: fields.port,
                    username: fields.username,
                    password: fields.password,
                    schema: fields.schema,
                    driver: fields.driver,
                },
                prompter,
                replace,
                fields.yes,
            )?;
            println!("已保存数据库连接 {name}。运行 dm db list 查看连接。");
        }
        DbCommand::Edit { name, fields } => edit::edit(context, &name, fields, prompter)?,
        DbCommand::Doctor { json } => return settings::doctor(context, json),
        DbCommand::Config { command } => settings::config(context, command)?,
        DbCommand::List { json } => {
            let connections = load_connections(context)?;
            if json {
                println!("{}", render_json(&connections)?);
            } else {
                let table = render_table(&connections);
                if table.is_empty() {
                    println!("尚无数据库连接。运行 `dm db add <name>` 添加连接。");
                } else {
                    println!("{table}");
                }
            }
        }
        DbCommand::Remove { name, yes } => {
            crate::find_connection(context, &name)?;
            dm_plugin_support::interaction::confirm(prompter, yes, &format!("删除连接 {name}？"))?;
            remove_connection(context, &name)?;
            println!("已删除数据库连接 {name}");
        }
        DbCommand::Export {
            file,
            include_passwords,
        } => transfer::export(context, file, include_passwords, prompter)?,
        DbCommand::Import { file, replace } => transfer::import(context, file, replace, prompter)?,
        DbCommand::Test { name } => {
            let name = dm_plugin_support::interaction::select_name(
                name,
                &load_connections(context)?
                    .into_iter()
                    .map(|entry| entry.name)
                    .collect::<Vec<_>>(),
                prompter,
            )?;
            query::test(context, factory, &name)?;
        }
        DbCommand::Exec { name, sql, file } => query::exec(context, factory, &name, sql, file)?,
    }
    Ok(0)
}
