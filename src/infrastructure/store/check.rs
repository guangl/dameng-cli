//! `dm install --check`: validate an installation source without installing.
//!
//! The check resolves the source, validates the manifest and applies the same
//! conflict rules and prebuilt lookups as the installer, but it never runs
//! hooks and never writes to the plugin store or the plugins directory.

use anyhow::{Context, Result, ensure};

use crate::Manifest;

use super::{InstallMode, PluginStore, ResolvedSource, resolve_source, try_download_prebuilt};

/// What `dm install --check` verified, so the user can decide to install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallPreview {
    pub name: String,
    pub version: String,
    /// Source recorded in the store after a successful installation.
    pub source: Option<String>,
    /// Resolved Git commit; `None` for a local package directory.
    pub revision: Option<String>,
    /// Whether an installed plugin of the same name would be replaced.
    pub replacing: bool,
    /// Whether the executable comes from a published prebuilt release.
    pub prebuilt: bool,
}

impl PluginStore {
    /// Report what installing this source would do, without changing anything.
    ///
    /// Remote sources are downloaded and checksum-verified exactly like an
    /// installation, so a passing check means the installation can proceed.
    pub fn check_install(
        &self,
        source: &str,
        revision: Option<&str>,
        replace: bool,
    ) -> Result<InstallPreview> {
        let resolved = resolve_source(source, revision, self.progress_enabled())?;
        let manifest = Manifest::read(&resolved.path)?;
        // The installer creates the plugins directory first; a check must not,
        // so the guard only applies once that directory exists.
        if let Ok(plugins) = std::fs::canonicalize(self.plugins()) {
            ensure!(
                !plugins.starts_with(&resolved.path),
                "Plugin source must not contain the plugin store"
            );
        }
        let mode = if replace {
            InstallMode::Replace
        } else {
            InstallMode::New
        };
        let replacing = super::conflict::resolve(
            self.installed(&manifest.name)?,
            &self.plugins().join(&manifest.name),
            &manifest.name,
            mode,
        )?;
        let prebuilt = check_executable(self, &resolved, &manifest)?;
        Ok(InstallPreview {
            name: manifest.name,
            version: manifest.version,
            source: resolved.recorded_source,
            revision: resolved.revision,
            replacing,
            prebuilt,
        })
    }

    /// Whether the store already records this plugin.
    pub(crate) fn installed(&self, name: &str) -> Result<bool> {
        Ok(self.connect()?.query_row(
            "SELECT EXISTS(SELECT 1 FROM installed_plugins WHERE name = ?1)",
            [name],
            |row| row.get::<_, bool>(0),
        )?)
    }
}

/// Confirm the executable an installation would publish: a binary inside the
/// package directory, or a checksum-verified prebuilt download.
fn check_executable(
    store: &PluginStore,
    resolved: &ResolvedSource,
    manifest: &Manifest,
) -> Result<bool> {
    if resolved.path.join(manifest.executable_name()).is_file() {
        return Ok(false);
    }
    if resolved
        .recorded_source
        .as_deref()
        .is_none_or(|source| source.starts_with("https://"))
    {
        let directory = tempfile::tempdir()?;
        try_download_prebuilt(
            resolved.recorded_source.as_deref(),
            manifest,
            resolved.source_ref.as_deref(),
            &directory.path().join(manifest.executable_name()),
            store.progress_enabled(),
        )
        .context("Source builds are disabled; install a prebuilt plugin release")?;
        return Ok(true);
    }
    // A local package without a built binary: the host never compiles sources,
    // so name the exact file the user has to build and copy.
    anyhow::bail!(
        "Local plugin package has no {} binary; build the plugin and copy it next to {}",
        manifest.executable_name(),
        crate::MANIFEST_FILE
    )
}
