use anyhow::{Context, Result, bail, ensure};
use log::info;
use rusqlite::params;
use std::{fs, path::Path};

use crate::Manifest;

use super::{InstallMode, PluginStore, sha256_file, try_download_prebuilt};

impl PluginStore {
    pub(crate) fn install_directory(
        &self,
        source: &Path,
        recorded_source: Option<String>,
        revision: Option<String>,
        source_ref: Option<String>,
        mode: InstallMode,
    ) -> Result<Manifest> {
        self.install_prepared(source, recorded_source, revision, source_ref, mode, None)
    }

    /// Publish either a prebuilt package or an explicitly built executable.
    pub(crate) fn install_prepared(
        &self,
        source: &Path,
        recorded_source: Option<String>,
        revision: Option<String>,
        source_ref: Option<String>,
        mode: InstallMode,
        built_binary: Option<&Path>,
    ) -> Result<Manifest> {
        let source = fs::canonicalize(source)?;
        let manifest = Manifest::read(&source)?;
        info!(
            "installing plugin {} {} from {}",
            manifest.name,
            manifest.version,
            source.display()
        );
        fs::create_dir_all(self.plugins())?;
        let plugins = fs::canonicalize(self.plugins())?;
        ensure!(
            !plugins.starts_with(&source),
            "Plugin source must not contain the plugin store"
        );
        let destination = plugins.join(&manifest.name);
        let mut connection = self.connect()?;
        let installed = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM installed_plugins WHERE name = ?1)",
            [&manifest.name],
            |row| row.get::<_, bool>(0),
        )?;
        // The same rules back "dm install --check", so a preview cannot pass
        // while the installation itself would be refused.
        let replacing = super::conflict::resolve(installed, &destination, &manifest.name, mode)?;
        let stage = tempfile::Builder::new()
            .prefix(".install-")
            .tempdir_in(&plugins)?;
        let package = stage.path().join("package");
        let prebuilt = stage.path().join("prebuilt");
        let local_binary = built_binary
            .map(Path::to_path_buf)
            .unwrap_or_else(|| source.join(manifest.executable_name()));
        if local_binary.is_file() {
            fs::copy(&local_binary, &prebuilt).context("Copy local prebuilt plugin")?;
        } else if recorded_source
            .as_deref()
            .is_none_or(|source| source.starts_with("https://"))
        {
            try_download_prebuilt(
                recorded_source.as_deref(),
                &manifest,
                source_ref.as_deref(),
                &prebuilt,
                self.progress_enabled(),
            )
            .context("Source builds are disabled by default; install a prebuilt plugin release or use dm install --build")?;
        } else {
            // A local package that has no built binary: default installation never compiles
            // sources, so name the exact file the user has to build and copy.
            bail!(
                "Local plugin package has no {} binary; use dm install --build or build the plugin and copy it next to {}",
                manifest.executable_name(),
                crate::MANIFEST_FILE
            );
        }
        if let Some(hook) = manifest.hooks.pre_install.as_deref() {
            self.run_hook(&source, hook, "pre-install", &manifest)?;
        }
        fs::create_dir(&package)?;
        fs::copy(&prebuilt, package.join(manifest.executable_name()))
            .context("Copy plugin binary")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                package.join(manifest.executable_name()),
                fs::Permissions::from_mode(0o755),
            )?;
        }
        fs::write(
            package.join(crate::MANIFEST_FILE),
            toml::to_string(&manifest)?,
        )?;
        manifest.entrypoint(&package)?;
        self.copy_hooks(&source, &package, &manifest)?;
        let checksum = sha256_file(&package.join(manifest.executable_name()))?;
        let previous = stage.path().join("previous");
        if replacing {
            fs::rename(&destination, &previous).context("Stage previous plugin version")?;
        }
        if let Err(error) = fs::rename(&package, &destination) {
            if replacing {
                let _ = fs::rename(&previous, &destination);
            }
            return Err(error).context("Publish installed plugin");
        }
        if let Some(hook) = manifest.hooks.post_install.as_deref()
            && let Err(error) = self.run_hook(&destination, hook, "post-install", &manifest)
        {
            let _ = fs::remove_dir_all(&destination);
            if replacing {
                let _ = fs::rename(&previous, &destination);
            }
            return Err(error).context("Run post-install hook");
        }
        let transaction = connection.transaction()?;
        let database_result = match installed {
            false => transaction.execute(
                "INSERT INTO installed_plugins
                 (name, manifest, source, revision, source_ref, checksum)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    manifest.name,
                    toml::to_string(&manifest)?,
                    recorded_source,
                    revision,
                    source_ref,
                    checksum
                ],
            ),
            true => transaction.execute(
                "UPDATE installed_plugins
                 SET manifest = ?2, source = ?3, revision = ?4, source_ref = ?5, checksum = ?6,
                     installed_at = unixepoch()
                 WHERE name = ?1",
                params![
                    manifest.name,
                    toml::to_string(&manifest)?,
                    recorded_source,
                    revision,
                    source_ref,
                    checksum
                ],
            ),
        };
        if let Err(error) = database_result.and_then(|_| transaction.commit()) {
            let _ = fs::remove_dir_all(&destination);
            if replacing {
                let _ = fs::rename(&previous, &destination);
            }
            return Err(error).context("Record installed plugin in SQLite");
        }
        Ok(manifest)
    }
}
