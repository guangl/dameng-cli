//! Installation conflict rules shared by `install` and its `--check` preview.
//!
//! Both paths have to agree on whether an installation may proceed, so the
//! rules live here instead of being duplicated in the installer.

use anyhow::{Result, ensure};
use std::{fs, path::Path};

use super::InstallMode;

/// Decide whether an installation may proceed, returning whether it replaces an
/// installed plugin. Only reads the filesystem and the store.
///
/// Host directory names are refused for new plugins only; an installed one
/// stays operable so it can be inspected, migrated or removed.
pub(super) fn resolve(
    installed: bool,
    destination: &Path,
    name: &str,
    mode: InstallMode,
) -> Result<bool> {
    if !installed {
        crate::plugin::manifest::validate_new_name(name)?;
    }
    let missing_on_disk = fs::symlink_metadata(destination)
        .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound);
    let present_on_disk = fs::symlink_metadata(destination).is_ok_and(|metadata| metadata.is_dir());
    match mode {
        InstallMode::New => {
            ensure!(
                !installed,
                "Plugin '{name}' is already installed; run dm update to upgrade it, or dm install --replace to replace this installation"
            );
            ensure!(
                missing_on_disk,
                "Plugin '{name}' already exists on disk; run dm doctor"
            );
            Ok(false)
        }
        InstallMode::Update => {
            ensure!(installed, "Plugin '{name}' is not installed");
            ensure!(
                present_on_disk,
                "Installed plugin '{name}' is missing or invalid; run dm doctor"
            );
            Ok(true)
        }
        // Replacement may also bring the first installation of a plugin, so an
        // installed plugin has to exist on disk, while a new one must not hit a
        // leftover directory.
        InstallMode::Replace => {
            if installed {
                ensure!(
                    present_on_disk,
                    "Installed plugin '{name}' is missing or invalid; run dm doctor"
                );
            } else {
                ensure!(
                    missing_on_disk,
                    "Plugin '{name}' already exists on disk; run dm doctor"
                );
            }
            Ok(installed)
        }
    }
}
