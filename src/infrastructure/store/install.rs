use anyhow::{Result, ensure};
use log::info;
use std::{fs, path::Path};

use crate::Manifest;

use super::{InstallMode, PluginStore, checkout_git};

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
        if Path::new(source).is_dir() {
            ensure!(
                revision.is_none(),
                "--rev is only supported for HTTPS Git sources"
            );
            let canonical = fs::canonicalize(source)?;
            return self.install_directory(
                &canonical,
                Some(canonical.display().to_string()),
                None,
                None,
                mode,
            );
        }
        ensure!(
            source.starts_with("https://") && source.len() > 8,
            "Source must be a local plugin directory or HTTPS Git repository URL"
        );
        let (checkout, destination, resolved_revision) =
            checkout_git(source, revision, self.progress_enabled())?;
        let result = self.install_directory(
            &destination,
            Some(source.to_owned()),
            Some(resolved_revision),
            revision.map(str::to_owned),
            mode,
        );
        drop(checkout);
        result
    }
}
