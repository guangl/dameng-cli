use anyhow::{Result, ensure};
use log::info;
use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::Manifest;

use super::{InstallMode, PluginStore, checkout_git};

/// An installation source resolved to the package directory to read.
pub(crate) struct ResolvedSource {
    /// Keeps a temporary Git checkout alive while the package is read.
    _checkout: Option<tempfile::TempDir>,
    /// Directory holding the plugin manifest and its executable.
    pub(crate) path: PathBuf,
    /// Source recorded in the store after a successful installation.
    pub(crate) recorded_source: Option<String>,
    /// Resolved Git commit; `None` for a local package directory.
    pub(crate) revision: Option<String>,
    /// Ref requested with `--rev`, kept for update checks.
    pub(crate) source_ref: Option<String>,
}

/// Turn a source argument into the directory to install from.
///
/// Local directories are canonicalized in place; HTTPS Git URLs are cloned into
/// a temporary checkout that lives as long as the returned value. Both
/// `install` and its `--check` preview resolve sources through here so they
/// cannot disagree about what a source means.
pub(crate) fn resolve_source(
    source: &str,
    revision: Option<&str>,
    progress: bool,
) -> Result<ResolvedSource> {
    if Path::new(source).is_dir() {
        ensure!(
            revision.is_none(),
            "--rev is only supported for HTTPS Git sources"
        );
        let canonical = fs::canonicalize(source)?;
        return Ok(ResolvedSource {
            _checkout: None,
            path: canonical.clone(),
            recorded_source: Some(canonical.display().to_string()),
            revision: None,
            source_ref: None,
        });
    }
    ensure!(
        source.starts_with("https://") && source.len() > 8,
        "Source must be a local plugin directory or HTTPS Git repository URL"
    );
    let (checkout, path, resolved_revision) = checkout_git(source, revision, progress)?;
    Ok(ResolvedSource {
        _checkout: Some(checkout),
        path,
        recorded_source: Some(source.to_owned()),
        revision: Some(resolved_revision),
        source_ref: revision.map(str::to_owned),
    })
}

impl PluginStore {
    pub fn install(&self, source: &str) -> Result<Manifest> {
        self.install_with_revision(source, None, false)
    }

    /// Install a prebuilt plugin, optionally replacing an installed plugin of
    /// the same name.
    ///
    /// Replacing keeps the plugin's config/data/cache directories and stages the
    /// previous version exactly like `update`; it exists for installers that hold
    /// a verified package directory, where `update` cannot work because the
    /// recorded source of the previous installation may be gone.
    pub fn install_with_revision(
        &self,
        source: &str,
        revision: Option<&str>,
        replace: bool,
    ) -> Result<Manifest> {
        info!("install source={source} rev={}", revision.unwrap_or("-"));
        let mode = if replace {
            InstallMode::Replace
        } else {
            InstallMode::New
        };
        let ResolvedSource {
            _checkout,
            path,
            recorded_source,
            revision,
            source_ref,
        } = resolve_source(source, revision, self.progress_enabled())?;
        let result = self.install_directory(&path, recorded_source, revision, source_ref, mode);
        drop(_checkout);
        result
    }
}
