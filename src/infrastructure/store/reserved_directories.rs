//! Recover narrowly scoped plugin directories stranded in a host-owned root.
use anyhow::{Context, Result, ensure};
use std::{collections::BTreeSet, fs, path::PathBuf};

use super::{PluginStore, RESERVED_HOME_ENTRIES, directories::is_empty_directory};

impl PluginStore {
    pub(crate) fn reserved_sources_for_doctor(
        &self,
        repair: bool,
        issues: &mut Vec<String>,
        repairs: &mut Vec<String>,
    ) -> Result<BTreeSet<PathBuf>> {
        let connection = self.completion_connection()?;
        let mut statement = connection.prepare(
            "SELECT name FROM installed_plugins UNION SELECT name FROM retained_plugin_data",
        )?;
        let names = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut protected = BTreeSet::new();
        for name in names
            .into_iter()
            .filter(|name| RESERVED_HOME_ENTRIES.contains(&name.as_str()))
        {
            for (source, _) in self.reserved_directory_moves(&name)? {
                issues.push(format!(
                    "reserved plugin directory needs recovery: {}",
                    source.display()
                ));
                protected.insert(source);
            }
            if repair {
                for restored in self.restore_reserved_directories(&name)? {
                    repairs.push(format!("restored reserved plugin directory {restored}"));
                }
            }
        }
        Ok(protected)
    }
    /// Read-only planning is also used by completion and the purge preview.
    pub(crate) fn reserved_directory_moves(&self, name: &str) -> Result<Vec<(PathBuf, PathBuf)>> {
        if !RESERVED_HOME_ENTRIES.contains(&name) {
            return Ok(Vec::new());
        }
        let connection = self.completion_connection()?;
        let mut statement = connection.prepare(
            "SELECT name FROM installed_plugins UNION SELECT name FROM retained_plugin_data",
        )?;
        let owners = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<BTreeSet<_>>>()?;
        let mut moves = Vec::new();
        for (kind, target) in ["config", "data", "cache"]
            .into_iter()
            .zip(self.legacy_per_plugin_directories(name))
        {
            let source = self.home.join(name).join(kind);
            if source == target || !source.is_dir() {
                continue;
            }
            // For example data/cache may be the cache plugin's legacy data;
            // plugins/data and backups/data may be its package or backup.
            if matches!(name, "config" | "data" | "cache" | "plugins" | "backups")
                && owners.contains(kind)
            {
                continue;
            }
            if name == "plugins" && crate::Manifest::read(&source).is_ok() {
                continue;
            }
            if name == "backups"
                && fs::read_dir(&source)?.any(|entry| {
                    entry.is_ok_and(|entry| crate::Manifest::read(&entry.path()).is_ok())
                })
            {
                continue;
            }
            ensure!(
                fs::symlink_metadata(&source)?.is_dir(),
                "Refusing to restore symlink directory {}",
                source.display()
            );
            ensure!(
                !self.contains_host_log_directory(&source),
                "Move the host log directory out of {} before restoring '{name}'",
                source.display()
            );
            if target.exists() {
                ensure!(
                    fs::symlink_metadata(&target)?.is_dir(),
                    "Refusing to restore into non-directory {}",
                    target.display()
                );
                if !is_empty_directory(&target) {
                    ensure!(
                        is_empty_directory(&source),
                        "Both {} and {} contain data for '{name}'; reconcile them manually before retrying",
                        source.display(),
                        target.display()
                    );
                    continue;
                }
            }
            moves.push((source, target));
        }
        Ok(moves)
    }

    pub(crate) fn restore_reserved_directories(&self, name: &str) -> Result<Vec<String>> {
        let mut restored = Vec::new();
        // Validate the whole plan before moving anything: ambiguous ownership
        // or a populated destination must never lead to overwritten data.
        for (source, target) in self.reserved_directory_moves(name)? {
            fs::create_dir_all(target.parent().expect("plugin directory parent"))?;
            if target.exists() {
                fs::remove_dir(&target)
                    .with_context(|| format!("Remove empty {}", target.display()))?;
            }
            fs::rename(&source, &target)
                .with_context(|| format!("Restore {} to {}", source.display(), target.display()))?;
            restored.push(format!("{} -> {}", source.display(), target.display()));
        }
        Ok(restored)
    }
}
