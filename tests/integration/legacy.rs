//! Helpers for the older per-kind plugin directory layout.
use std::path::{Path, PathBuf};

/// Directories an older host used for one plugin, with a sample file name each.
pub(crate) const LEGACY_PLUGIN_DIRECTORIES: [(&str, &str); 3] = [
    ("config", "config.toml"),
    ("data", "connections.sqlite3"),
    ("cache", "session"),
];

/// Legacy directories of the `probe` fixture plugin.
pub(crate) fn legacy_plugin_directories(home: &Path) -> Vec<PathBuf> {
    LEGACY_PLUGIN_DIRECTORIES
        .iter()
        .map(|(kind, _)| home.join(kind).join("probe"))
        .collect()
}

/// The fixture plugin prints the data directory the host handed it.
pub(crate) fn reported_data_dir(output: &str) -> PathBuf {
    let line = output
        .lines()
        .find(|line| line.starts_with("data="))
        .unwrap_or_else(|| panic!("no data directory in {output}"));
    PathBuf::from(line.trim_start_matches("data="))
}
