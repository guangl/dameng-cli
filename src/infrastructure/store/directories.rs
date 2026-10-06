//! Where a plugin keeps its own files inside the plugin home directory.
use anyhow::{Context, Result};
use log::debug;
use std::{
    fs,
    path::{Path, PathBuf},
};

use super::PluginStore;

/// Entries inside the plugin home directory that never belong to a plugin.
///
/// `config`, `data` and `cache` are the roots of the older layout that grouped
/// directories by kind; they stay reserved so old and new installations cannot
/// collide.
pub(crate) const RESERVED_HOME_ENTRIES: &[&str] =
    &["config", "data", "cache", "logs", "plugins", "backups"];

/// Whether a directory exists and holds no entry; unreadable directories count
/// as non-empty so migration never moves data on top of unknown content.
pub(super) fn is_empty_directory(path: &std::path::Path) -> bool {
    fs::read_dir(path).is_ok_and(|mut entries| entries.next().is_none())
}

impl PluginStore {
    /// Per-plugin directories: configuration, data and cache.
    pub fn plugin_directories(&self, name: &str) -> [PathBuf; 3] {
        self.per_plugin_directories(name)
    }

    /// Per-plugin directories, grouped by plugin: config, data and cache live
    /// below `<DM_PLUGIN_HOME>/<name>/` so one plugin owns one subtree.
    pub(crate) fn per_plugin_directories(&self, name: &str) -> [PathBuf; 3] {
        // Older releases allowed these names. Their grouped root belongs to
        // the host, so keep using the original layout for those installations.
        if RESERVED_HOME_ENTRIES.contains(&name) {
            return self.legacy_per_plugin_directories(name);
        }
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
    /// exactly once. A destination that already holds data wins and the legacy
    /// directory stays untouched; an empty destination (for example a log
    /// directory a user pointed at that path) receives the legacy entries so
    /// nothing is shadowed.
    pub(crate) fn migrate_per_plugin_directories(&self, name: &str) -> Result<Vec<String>> {
        if RESERVED_HOME_ENTRIES.contains(&name) {
            return self.restore_reserved_directories(name);
        }
        let mut migrated = Vec::new();
        for (old, new) in self
            .legacy_per_plugin_directories(name)
            .into_iter()
            .zip(self.per_plugin_directories(name))
        {
            if !old.is_dir() {
                continue;
            }
            // A host-created placeholder (for example a log directory the user
            // pointed at this path) must not shadow plugin data: an empty
            // destination receives the legacy entries.
            if new.exists() {
                if !is_empty_directory(&new) {
                    continue;
                }
                for entry in fs::read_dir(&old)? {
                    let entry = entry?;
                    fs::rename(entry.path(), new.join(entry.file_name())).with_context(|| {
                        format!("Move {} into {}", entry.path().display(), new.display())
                    })?;
                }
                fs::remove_dir(&old).with_context(|| format!("Remove empty {}", old.display()))?;
            } else {
                if let Some(parent) = new.parent() {
                    fs::create_dir_all(parent)
                        .with_context(|| format!("Create {}", parent.display()))?;
                }
                if let Err(error) = fs::rename(&old, &new) {
                    // Two processes can migrate the same plugin at once; the
                    // loser sees the source disappear and must not fail the
                    // launch it was asked for.
                    if error.kind() != std::io::ErrorKind::NotFound || !new.is_dir() {
                        return Err(error).with_context(|| {
                            format!(
                                "Move plugin directory {} to {}; make sure DM_PLUGIN_HOME is writable or move it manually",
                                old.display(),
                                new.display()
                            )
                        });
                    }
                    continue;
                }
            }
            debug!("migrated {} to {}", old.display(), new.display());
            migrated.push(format!("{} -> {}", old.display(), new.display()));
        }
        Ok(migrated)
    }

    /// Protect the log directory, its ancestors and its contents from cleanup.
    pub(crate) fn protects_host_logs(&self, path: &Path) -> bool {
        // Protect both the actual storage and the ancestors containing a
        // symlink needed to reach it (which may point outside the host home).
        [false, true].into_iter().any(|resolve_links| {
            let logs = normalized_path(&self.log_directory(), resolve_links);
            let candidate = normalized_path(path, resolve_links);
            logs.starts_with(&candidate) || candidate.starts_with(&logs)
        })
    }

    pub(crate) fn contains_host_log_directory(&self, path: &Path) -> bool {
        [false, true].into_iter().any(|resolve_links| {
            normalized_path(&self.log_directory(), resolve_links)
                .starts_with(normalized_path(path, resolve_links))
        })
    }
}

/// Resolve existing symlinks and normalize dot components, including paths
/// whose final components have not been created yet.
fn normalized_path(path: &Path, resolve_links: bool) -> PathBuf {
    let mut resolved = if path.is_absolute() {
        PathBuf::new()
    } else {
        std::env::current_dir().unwrap_or_default()
    };
    for component in path.components() {
        if component == std::path::Component::ParentDir {
            resolved.pop();
        } else {
            resolved.push(component);
            if resolve_links && let Ok(canonical) = fs::canonicalize(&resolved) {
                resolved = canonical;
            }
        }
    }
    resolved
}
