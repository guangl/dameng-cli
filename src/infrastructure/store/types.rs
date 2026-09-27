use serde::Serialize;

use crate::Manifest;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PluginInfo {
    pub manifest: Manifest,
    pub source: Option<String>,
    pub revision: Option<String>,
    pub source_ref: Option<String>,
    pub checksum: String,
    pub installed_at: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DoctorReport {
    pub issues: Vec<String>,
    pub repairs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct UpdateStatus {
    pub name: String,
    pub installed_version: String,
    pub available_version: Option<String>,
    pub update_available: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum InstallMode {
    /// Refuse to touch a plugin of the same name.
    New,
    /// Replace a plugin that is already installed.
    Update,
    /// Replace an installed plugin, or install it when it is not installed yet.
    Replace,
}
