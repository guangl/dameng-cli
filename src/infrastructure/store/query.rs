use anyhow::{Context, Result, ensure};
use rusqlite::OptionalExtension;
use std::{fs, path::PathBuf};

use crate::{Manifest, plugin::manifest::validate_name};

use super::{PluginInfo, PluginStore, sha256_file};

impl PluginStore {
    pub fn list(&self) -> Result<Vec<Manifest>> {
        let connection = self.connect()?;
        let names = {
            let mut statement =
                connection.prepare("SELECT name FROM installed_plugins ORDER BY name")?;
            statement
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        names
            .into_iter()
            .map(|name| self.load(&name).map(|(_, manifest)| manifest))
            .collect()
    }

    pub fn list_info(&self) -> Result<Vec<PluginInfo>> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT manifest, source, revision, source_ref, checksum, installed_at
             FROM installed_plugins ORDER BY name",
        )?;
        let rows = statement.query_map([], read_info)?;
        rows.map(|row| decode_info(row?)).collect()
    }

    pub fn info(&self, name: &str) -> Result<PluginInfo> {
        validate_name(name)?;
        let values = self
            .connect()?
            .query_row(
                "SELECT manifest, source, revision, source_ref, checksum, installed_at
                 FROM installed_plugins WHERE name = ?1",
                [name],
                read_info,
            )
            .optional()?
            .with_context(|| format!("Plugin '{name}' is not installed"))?;
        decode_info(values)
    }

    pub fn verify(&self, name: Option<&str>) -> Result<Vec<String>> {
        let plugins = if let Some(name) = name {
            vec![self.info(name)?]
        } else {
            self.list_info()?
        };
        let mut verified = Vec::new();
        for info in plugins {
            let (root, manifest) = self.load(&info.manifest.name)?;
            let checksum = sha256_file(&manifest.entrypoint(&root)?)?;
            ensure!(
                !info.checksum.is_empty() && checksum == info.checksum,
                "Plugin '{}' checksum mismatch",
                manifest.name
            );
            verified.push(manifest.name);
        }
        Ok(verified)
    }

    #[doc(hidden)]
    pub fn load(&self, name: &str) -> Result<(PathBuf, Manifest)> {
        validate_name(name)?;
        let stored = self
            .connect()?
            .query_row(
                "SELECT manifest FROM installed_plugins WHERE name = ?1",
                [name],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .with_context(|| {
                format!("Plugin '{name}' is not installed; use dm install <source>")
            })?;
        let stored = Manifest::from_toml(&stored).context("Invalid manifest in SQLite store")?;
        let root = self.plugins().join(name);
        let metadata = fs::symlink_metadata(&root)
            .with_context(|| format!("Installed plugin '{name}' is missing from disk"))?;
        ensure!(
            metadata.is_dir(),
            "Installed plugin must be a regular directory"
        );
        let root = fs::canonicalize(root)?;
        let manifest = Manifest::read(&root)?;
        ensure!(
            manifest == stored,
            "Installed plugin manifest differs from SQLite metadata"
        );
        Ok((root, manifest))
    }
}

type StoredInfo = (
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
    i64,
);

fn read_info(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredInfo> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
    ))
}

fn decode_info(values: StoredInfo) -> Result<PluginInfo> {
    Ok(PluginInfo {
        manifest: Manifest::from_toml(&values.0).context("Invalid manifest in SQLite store")?,
        source: values.1,
        revision: values.2,
        source_ref: values.3,
        checksum: values.4,
        installed_at: values.5,
    })
}
