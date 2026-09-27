use serde::Serialize;

/// Release targets the host publishes. Used to reject `--target` and
/// `update_target` typos before any download starts.
pub const SUPPORTED_TARGETS: &[&str] = &[
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-unknown-linux-musl",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
];

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SelfUpdateResult {
    pub current_version: String,
    pub available_version: String,
    pub updated: bool,
}

/// Options for [`crate::self_update_with_options`].
///
/// `repository` carries the value resolved from the configuration file, so the
/// library never has to guess which source the caller already consulted.
#[derive(Debug, Default, Clone, Copy)]
pub struct SelfUpdateOptions<'a> {
    /// Install this exact version instead of the latest release.
    pub version: Option<&'a str>,
    /// Report the available version without installing anything.
    pub check_only: bool,
    /// Reinstall or downgrade even when the requested version is not newer.
    pub force: bool,
    /// Override the release target triple for this run.
    pub target: Option<&'a str>,
    /// GitHub `owner/repository`; falls back to `DM_UPDATE_REPOSITORY`, then the default.
    pub repository: Option<&'a str>,
}
