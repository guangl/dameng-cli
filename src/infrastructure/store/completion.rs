use super::PluginStore;
use crate::{Manifest, plugin::manifest::validate_name};
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OpenFlags};
use std::{fs, path::PathBuf};

impl PluginStore {
    pub(crate) fn completion_connection(&self) -> Result<Connection> {
        let connection = Connection::open_with_flags(
            self.home.join("store.sqlite3"),
            OpenFlags::SQLITE_OPEN_READ_ONLY,
        )?;
        connection.busy_timeout(std::time::Duration::from_millis(20))?;
        Ok(connection)
    }
    /// Read names without creating or migrating the plugin store.
    pub fn completion_names(&self) -> Result<Vec<String>> {
        let connection = self.completion_connection()?;
        let mut statement =
            connection.prepare("SELECT name FROM installed_plugins ORDER BY name")?;
        Ok(statement
            .query_map([], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?)
    }
    pub(crate) fn load_completion(&self, name: &str) -> Result<(PathBuf, Manifest)> {
        validate_name(name)?;
        let text: String = self.completion_connection()?.query_row(
            "SELECT manifest FROM installed_plugins WHERE name = ?1",
            [name],
            |row| row.get(0),
        )?;
        let stored = Manifest::from_toml(&text)?;
        let root = self.plugins().join(name);
        ensure!(fs::symlink_metadata(&root)?.is_dir(), "插件目录无效");
        let root = fs::canonicalize(root).context("读取插件目录")?;
        let manifest = Manifest::read(&root)?;
        ensure!(manifest == stored, "插件清单与安装记录不一致");
        Ok((root, manifest))
    }
}
