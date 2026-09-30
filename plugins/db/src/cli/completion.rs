use super::args::Cli;
use anyhow::Result;
use clap::CommandFactory;
use dm_plugin_sdk::Context;
use rusqlite::{Connection, OpenFlags};

pub(super) fn handle(context: &Context) -> Result<bool> {
    if context.args.first().is_none_or(|arg| arg != "__complete") {
        return Ok(false);
    }
    // Completion must never create a store, machine key, or connection.
    let names = (|| -> Result<Vec<String>> {
        let connection = Connection::open_with_flags(
            context.data_dir.join("connections.sqlite3"),
            OpenFlags::SQLITE_OPEN_READ_ONLY,
        )?;
        connection.busy_timeout(std::time::Duration::from_millis(20))?;
        let mut statement = connection.prepare("SELECT name FROM connections ORDER BY name")?;
        Ok(statement
            .query_map([], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?)
    })()
    .unwrap_or_default();
    let words: Vec<_> = context
        .args
        .iter()
        .skip(1)
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    for value in dm_plugin_support::completion::candidates(Cli::command(), &words, &names) {
        println!("{value}");
    }
    Ok(true)
}
