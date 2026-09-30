use anyhow::{Context, Result, ensure};
use log::info;
use std::fs;

use crate::{Manifest, plugin::manifest::validate_name};

use super::PluginStore;

impl PluginStore {
    /// Compatibility API: remove the package and its data.
    pub fn uninstall(&self, name: &str) -> Result<()> {
        self.uninstall_with_options(name, true)
    }

    /// Remove a package, retaining its data unless purge was explicitly chosen.
    pub fn uninstall_with_options(&self, name: &str, purge: bool) -> Result<()> {
        info!("uninstalling plugin {name}");
        validate_name(name)?;
        let connection = self.connect()?;
        let installed: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM installed_plugins WHERE name = ?1)",
            [name],
            |row| row.get(0),
        )?;
        if !installed && purge && self.has_retained_data(name)? {
            return self.purge_directories(name);
        }
        ensure!(installed, "Plugin '{name}' is not installed");
        let path = self.plugins().join(name);
        let metadata = fs::symlink_metadata(&path)
            .with_context(|| format!("Plugin '{name}' is not installed"))?;
        ensure!(
            metadata.is_dir(),
            "Installed plugin must be a regular directory"
        );
        let manifest = Manifest::read(&path).ok();
        if let Some(manifest) = manifest.as_ref() {
            if let Some(hook) = manifest.hooks.pre_uninstall.as_deref() {
                self.run_hook(&path, hook, "pre-uninstall", manifest)?;
            }
        }
        // Rename first so a database error can restore the complete installation.
        let stage = tempfile::Builder::new()
            .prefix(".remove-")
            .tempdir_in(self.plugins())?;
        let removed = stage.path().join("package");
        fs::rename(&path, &removed).context("Stage plugin removal")?;
        if let Some(manifest) = manifest.as_ref() {
            if let Some(hook) = manifest.hooks.post_uninstall.as_deref() {
                if let Err(error) = self.run_hook(&removed, hook, "post-uninstall", manifest) {
                    let _ = fs::rename(&removed, &path);
                    return Err(error).context("Run post-uninstall hook");
                }
            }
        }
        let result = (|| -> Result<()> {
            let transaction = connection.unchecked_transaction()?;
            // Keep the data discoverable until every purge path has been removed.
            // This also makes interrupted or failed purges safe to retry.
            transaction.execute(
                "INSERT OR IGNORE INTO retained_plugin_data (name) VALUES (?1)",
                [name],
            )?;
            transaction.execute("DELETE FROM installed_plugins WHERE name = ?1", [name])?;
            transaction.commit()?;
            Ok(())
        })();
        if let Err(error) = result {
            let _ = fs::rename(&removed, &path);
            return Err(error).context("Remove plugin metadata");
        }
        if !purge {
            return Ok(());
        }
        self.purge_directories(name)
    }

    /// True when uninstall kept data or a purge still needs to finish.
    pub fn has_retained_data(&self, name: &str) -> Result<bool> {
        validate_name(name)?;
        Ok(self.connect()?.query_row(
            "SELECT EXISTS(SELECT 1 FROM retained_plugin_data WHERE name = ?1)",
            [name],
            |row| row.get(0),
        )?)
    }
    /// Paths affected by a purge, including package backups.
    pub fn removal_paths(&self, name: &str) -> Vec<std::path::PathBuf> {
        self.per_plugin_directories(name)
            .into_iter()
            .chain(std::iter::once(self.backups().join(name)))
            .collect()
    }
    fn purge_directories(&self, name: &str) -> Result<()> {
        let mut cleanup_failures = Vec::new();
        for directory in self.removal_paths(name) {
            if let Err(error) = fs::remove_dir_all(&directory) {
                if error.kind() != std::io::ErrorKind::NotFound {
                    cleanup_failures.push(format!("{}: {error}", directory.display()));
                }
            }
        }
        ensure!(
            cleanup_failures.is_empty(),
            "Plugin '{name}' was uninstalled, but cleanup failed for: {}. Fix the reported paths, then retry `dm uninstall {name} --purge --yes`; remaining data is protected from `dm doctor --repair`",
            cleanup_failures.join("; ")
        );
        self.connect()?
            .execute("DELETE FROM retained_plugin_data WHERE name = ?1", [name])?;
        Ok(())
    }
}
