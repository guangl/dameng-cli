use crate::manifest::validate_name;
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::{fs, process::Command};

#[derive(Debug, Deserialize)]
struct RegistryIndexEntry {
    name: String,
    source: String,
}

// A remote registry index is a JSON array of { "name": "...", "source": "..." }
// objects, where source is an HTTPS Git repository URL.
pub fn parse_registry_index(bytes: &[u8]) -> Result<Vec<(String, String)>> {
    let entries: Vec<RegistryIndexEntry> = serde_json::from_slice(bytes)
        .context("Registry index must be a JSON array of { name, source } objects")?;
    let mut result = Vec::with_capacity(entries.len());
    for entry in entries {
        validate_name(&entry.name).with_context(|| {
            format!(
                "Registry index contains invalid plugin name '{}'",
                entry.name
            )
        })?;
        ensure!(
            entry.source.starts_with("https://") && entry.source.len() > 8,
            "Registry entry '{}' must use an HTTPS Git repository URL",
            entry.name
        );
        result.push((entry.name, entry.source));
    }
    Ok(result)
}

pub fn fetch_registry_index(url: &str) -> Result<Vec<(String, String)>> {
    ensure!(
        url.starts_with("https://") && url.len() > 8,
        "Registry index URL must use HTTPS"
    );
    let temp = tempfile::tempdir()?;
    let destination = temp.path().join("registry.json");
    let status = Command::new("curl")
        .args([
            "-fsSL",
            "--retry",
            "3",
            "--connect-timeout",
            "15",
            "--output",
        ])
        .arg(&destination)
        .arg(url)
        .status()
        .context("Fetch registry index; remote registry access requires curl")?;
    ensure!(status.success(), "Could not download registry index {url}");
    parse_registry_index(&fs::read(destination)?)
}
