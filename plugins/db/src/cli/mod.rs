//! Parsing and dispatch of the "dm db" subcommands.

mod add;
mod args;
mod query;
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
    let mut argv = vec![OsString::from("dm db")];
    argv.extend(context.args.iter().cloned());
    let cli = Cli::parse_from(argv);
    match cli.command {
        DbCommand::Add {
            name,
            host,
            port,
            username,
            password,
            schema,
            driver,
        } => {
            let name = add_connection(
                context,
                &AddRequest {
                    name,
                    host,
                    port,
                    username,
                    password,
                    schema,
                    driver,
                },
                prompter,
            )?;
            println!("Saved database connection {name}");
        }
        DbCommand::List { json } => {
            let connections = load_connections(context)?;
            if json {
                println!("{}", render_json(&connections)?);
            } else {
                let table = render_table(&connections);
                if table.is_empty() {
                    println!("No saved database connections. Run `dm db add <name>` to add one.");
                } else {
                    println!("{table}");
                }
            }
        }
        DbCommand::Remove { name } => {
            remove_connection(context, &name)?;
            println!("Removed database connection {name}");
        }
        DbCommand::Export {
            file,
            include_passwords,
        } => transfer::export(context, file, include_passwords, prompter)?,
        DbCommand::Import { file, replace } => transfer::import(context, file, replace, prompter)?,
        DbCommand::Test { name } => query::test(context, factory, &name)?,
        DbCommand::Exec { name, sql, file } => query::exec(context, factory, &name, sql, file)?,
    }
    Ok(0)
}
