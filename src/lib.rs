//! Process-based plugin host. Database functionality belongs in separate plugins.
mod manifest;
pub mod registry_index;
mod scaffold;
pub mod self_update;
mod store;

pub use manifest::{API_VERSION, MANIFEST_FILE, Manifest, SUPPORTED_API_VERSIONS};
pub use scaffold::scaffold_plugin;
pub use self_update::{
    SelfUpdateResult, cleanup_self_update_backup, self_update, self_update_with_options,
};
pub use store::{DoctorReport, PluginInfo, PluginStore, UpdateStatus};
