//! Process-based plugin host. Database functionality belongs in separate plugins.
mod infrastructure;
mod plugin;

/// Secure host self-update support.
pub mod self_update {
    pub use crate::infrastructure::self_update::*;
}

pub use infrastructure::store::{
    DoctorReport, PluginInfo, PluginStore, UpdateStatus, github_repository,
    prebuilt_target_label_for, progress_bar_for, release_tag_candidates, versions_differ,
};
pub use plugin::manifest::{API_VERSION, MANIFEST_FILE, Manifest, SUPPORTED_API_VERSIONS};
pub use self_update::{
    SelfUpdateResult, cleanup_self_update_backup, self_update, self_update_with_options,
};
