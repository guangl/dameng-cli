//! Parsing and dispatch of the "dm db" subcommands.

mod add;
mod cli;

use crate::commands::add::{AddRequest, add_connection};
use crate::commands::cli::{Cli, DbCommand};
use crate::config::load_config;
use crate::connection_string::connection_spec;
use crate::connections::{
    connection_password, find_connection, load_connections, remove_connection,
};
use crate::driver::PendingFactory;
use crate::export::{ExportDocument, export_document};
use crate::import::import_document;
use crate::list::{render_json, render_table};
use crate::private_file::write_private_file;
use crate::prompts::{Prompter, terminal_prompter};
use crate::sql::{format_result, read_sql};
use crate::types::{DEFAULT_PROBE, DatabaseFactory, Outcome};
use anyhow::{Context, Result};
use clap::Parser;
use dm_plugin_sdk::Context as PluginContext;
use std::{ffi::OsString, fs, io::Write};

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
                // An empty store renders nothing at all, so "--json" is the
                // only form that reports an empty list explicitly.
                if !table.is_empty() {
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
        } => {
            let connections = load_connections(context)?;
            let document = export_document(context, connections, include_passwords, prompter)?;
            let json = serde_json::to_vec_pretty(&document)?;
            match file {
                Some(path) => {
                    write_private_file(&path, &json)?;
                    println!(
                        "Exported {} database connections to {}",
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
        DbCommand::Import { file, replace } => {
            let document: ExportDocument = serde_json::from_slice(
                &fs::read(&file).with_context(|| format!("Read {}", file.display()))?,
            )
            .with_context(|| format!("Parse database connection export {}", file.display()))?;
            let connections = import_document(context, document, replace, prompter)?;
            println!("Imported {connections} database connections");
        }
        DbCommand::Test { name } => {
            let config = load_config(context)?;
            let connection = find_connection(context, &name)?;
            let password = connection_password(context, &connection)?;
            let spec = connection_spec(&connection, &password, &config)?;
            let database = factory.open()?;
            let mut session = database.connect(&spec)?;
            session.run(config.connect.probe.as_deref().unwrap_or(DEFAULT_PROBE))?;
            println!("Database connection {name} is reachable");
        }
        DbCommand::Exec { name, sql, file } => {
            // The SQL is read before anything else so an empty statement fails
            // fast and without touching the store or the driver.
            let sql = read_sql(sql, file)?;
            let config = load_config(context)?;
            let connection = find_connection(context, &name)?;
            let password = connection_password(context, &connection)?;
            let spec = connection_spec(&connection, &password, &config)?;
            let database = factory.open()?;
            let mut session = database.connect(&spec)?;
            match session.run(&sql)? {
                Outcome::Rows(result) => print!("{}", format_result(&result)),
                Outcome::Affected(count) => println!("{count} rows affected"),
            }
        }
    }
    Ok(0)
}
