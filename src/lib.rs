//! Process-based plugin host. Database functionality belongs in separate plugins.
mod manifest;
mod store;

pub use manifest::{API_VERSION, MANIFEST_FILE, Manifest};
pub use store::PluginStore;
