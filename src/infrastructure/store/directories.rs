//! Where a plugin keeps its own files inside the plugin home directory.
use anyhow::{Context, Result};
use log::debug;
use std::{fs, path::PathBuf};

use super::PluginStore;

/// Entries inside the plugin home directory that never belong to a plugin.
///
/// `config`, `data` and `cache` are the roots of the older layout that grouped
/// directories by kind; they stay reserved so old and new installations cannot
/// collide.
pub(crate) const RESERVED_HOME_ENTRIES: &[&str] =
    &["config", "data", "cache", "logs", "plugins", "backups"];

impl PluginStore {
    /// Per-plugin directories: configuration, data and cache.
    pub fn plugin_directories(&self, name: &str) -> [PathBuf; 3] {
        self.per_plugin_directories(name)
    }

    /// Per-plugin directories, grouped by plugin: config, data and cache live
    /// below `<DM_PLUGIN_HOME>/<name>/` so one plugin owns one subtree.
    pub(crate) fn per_plugin_directories(&self, name: &str) -> [PathBuf; 3] {
        let root = self.home.join(name);
        [root.join("config"), root.join("data"), root.join("cache")]
    }

    /// Directories used by the layout that grouped by kind
    /// (`<DM_PLUGIN_HOME>/config/<name>` and friends).
    pub(crate) fn legacy_per_plugin_directories(&self, name: &str) -> [PathBuf; 3] {
        [
            self.home.join("config").join(name),
            self.home.join("data").join(name),
            self.home.join("cache").join(name),
        ]
    }

    /// Move directories written by the older layout into the grouped layout.
    ///
    /// A plugin can only read one location, so its directories are moved
    /// exactly once: an existing destination wins and a leftover legacy
    /// directory is left untouched instead of being merged or deleted.
    pub(crate) fn migrate_per_plugin_directories(&self, name: &str) -> Result<Vec<String>> {
        let mut migrated = Vec::new();
        for (old, new) in self
            .legacy_per_plugin_directories(name)
            .into_iter()
            .zip(self.per_plugin_directories(name))
        {
            if !old.is_dir() || new.exists() {
                continue;
            }
            if let Some(parent) = new.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("Create {}", parent.display()))?;
            }
            fs::rename(&old, &new).with_context(|| {
                format!(
                    "Move plugin directory {} to {}; make sure DM_PLUGIN_HOME is writable or move it manually",
                    old.display(),
                    new.display()
                )
            })?;
            debug!("migrated {} to {}", old.display(), new.display());
            migrated.push(format!("{} -> {}", old.display(), new.display()));
        }
        Ok(migrated)
    }
}
