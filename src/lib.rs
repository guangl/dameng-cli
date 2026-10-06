//! Process-based plugin host. Database functionality belongs in separate plugins.
/// Command-line interface, including the renderers used by its tests.
pub mod cli;
mod infrastructure;
mod plugin;

#[doc(hidden)]
pub mod support;

/// Host diagnostics: where the log file lives and how the backend starts.
pub mod logging {
    pub use crate::infrastructure::logging::*;
}

/// Secure host self-update support.
pub mod self_update {
    pub use crate::infrastructure::self_update::*;
}

pub use infrastructure::config::{CONFIG_FILE, Config, DEFAULT_LOG_FILTER, log_filter_from_env};
pub use infrastructure::store::{
    BuildOptions, DoctorReport, InstallPreview, PluginInfo, PluginStore, UpdateStatus,
    github_repository, home_from_env, prebuilt_target_label_for, progress_bar_for,
    release_tag_candidates, versions_differ,
};
pub use plugin::manifest::{API_VERSION, MANIFEST_FILE, Manifest, SUPPORTED_API_VERSIONS};
pub use self_update::{
    SUPPORTED_TARGETS, SelfUpdateOptions, SelfUpdateResult, cleanup_self_update_backup,
    self_update, self_update_with_options,
};
