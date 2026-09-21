//! Process-based plugin host. Database functionality belongs in separate plugins.
mod manifest;
pub mod registry;
mod scaffold;
mod store;
pub mod update;

pub use manifest::{API_VERSION, MANIFEST_FILE, Manifest, SUPPORTED_API_VERSIONS};
pub use scaffold::scaffold_plugin;
pub use store::{DoctorReport, PluginInfo, PluginStore, UpdateStatus};
pub use update::{SelfUpdateResult, cleanup_self_update_backup, self_update};
