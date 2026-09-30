//! Database commands and their shared connection setup.

use crate::domain::connection_string::connection_spec;
use crate::domain::sql::{format_result, read_sql};
use crate::domain::types::{ConnectionSpec, DEFAULT_PROBE, DatabaseFactory, Outcome};
use crate::storage::config::{DbConfig, load_config};
use crate::storage::connections::{connection_password, find_connection};
use anyhow::Result;
use dm_plugin_sdk::Context;
use std::path::PathBuf;

fn connection(context: &Context, name: &str, config: &DbConfig) -> Result<ConnectionSpec> {
    let connection = find_connection(context, name)?;
    let password = connection_password(context, &connection)?;
    connection_spec(&connection, &password, config)
}

pub(super) fn test(context: &Context, factory: &dyn DatabaseFactory, name: &str) -> Result<()> {
    let config = load_config(context)?;
    let spec = connection(context, name, &config)?;
    let database = factory.open()?;
    let mut session = database.connect(&spec)?;
    session.run(config.connect.probe.as_deref().unwrap_or(DEFAULT_PROBE))?;
    println!("Database connection {name} is reachable");
    Ok(())
}

pub(super) fn exec(
    context: &Context,
    factory: &dyn DatabaseFactory,
    name: &str,
    sql: Option<String>,
    file: Option<PathBuf>,
) -> Result<()> {
    // Invalid SQL must fail before reading settings, records or opening a driver.
    let sql = read_sql(sql, file)?;
    let config = load_config(context)?;
    let spec = connection(context, name, &config)?;
    let database = factory.open()?;
    let mut session = database.connect(&spec)?;
    match session.run(&sql)? {
        Outcome::Rows(result) => print!("{}", format_result(&result)),
        Outcome::Affected(count) => println!("{count} rows affected"),
    }
    Ok(())
}
