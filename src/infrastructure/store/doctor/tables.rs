use crate::infrastructure::store::STORE_TABLES;
use anyhow::Result;

/// Tables in the host store that the host does not own.
pub(super) fn foreign_store_tables(connection: &rusqlite::Connection) -> Result<Vec<String>> {
    let mut statement = connection.prepare(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
    )?;
    let names = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(names
        .into_iter()
        .filter(|name| !STORE_TABLES.contains(&name.as_str()))
        .collect())
}

/// Snapshot installation records before comparing packages on disk.
pub(super) fn installed_plugin_names(
    connection: &rusqlite::Connection,
) -> Result<std::collections::BTreeSet<String>> {
    let mut statement = connection.prepare("SELECT name FROM installed_plugins")?;
    Ok(statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<_>>()?)
}
