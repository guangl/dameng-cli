//! Process-based plugin host. Database functionality belongs in separate plugins.
mod infrastructure;
mod plugin;

/// Remote registry index parsing and retrieval.
pub mod registry_index {
    pub use crate::infrastructure::registry::*;
}

/// Secure host self-update support.
pub mod self_update {
    pub use crate::infrastructure::self_update::*;
}

pub use infrastructure::store::{DoctorReport, PluginInfo, PluginStore, UpdateStatus};
pub use plugin::manifest::{API_VERSION, MANIFEST_FILE, Manifest, SUPPORTED_API_VERSIONS};
pub use plugin::scaffold::scaffold_plugin;
pub use self_update::{
    SelfUpdateResult, cleanup_self_update_backup, self_update, self_update_with_options,
};
