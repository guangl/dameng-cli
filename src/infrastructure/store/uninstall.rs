use anyhow::{Context, Result, ensure};
use log::info;
use std::fs;

use crate::{Manifest, plugin::manifest::validate_name};

use super::PluginStore;

impl PluginStore {
    pub fn uninstall(&self, name: &str) -> Result<()> {
        info!("uninstalling plugin {name}");
        validate_name(name)?;
        let connection = self.connect()?;
        ensure!(
            connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM installed_plugins WHERE name = ?1)",
                [name],
                |row| row.get::<_, bool>(0),
            )?,
            "Plugin '{name}' is not installed"
        );
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
        if let Err(error) =
            connection.execute("DELETE FROM installed_plugins WHERE name = ?1", [name])
        {
            let _ = fs::rename(&removed, &path);
            return Err(error).context("Remove plugin metadata");
        }
        let mut cleanup_failures = Vec::new();
        for directory in self
            .per_plugin_directories(name)
            .into_iter()
            .chain(std::iter::once(self.backups().join(name)))
        {
            if let Err(error) = fs::remove_dir_all(&directory) {
                if error.kind() != std::io::ErrorKind::NotFound {
                    cleanup_failures.push(format!("{}: {error}", directory.display()));
                }
            }
        }
        ensure!(
            cleanup_failures.is_empty(),
            "Plugin '{name}' was uninstalled, but cleanup failed for: {}. Review the reported paths; `dm doctor --repair` can clean orphaned config/data/cache directories",
            cleanup_failures.join("; ")
        );
        Ok(())
    }
}
