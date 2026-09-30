//! Version checks by default; explicit plugin names or --all apply updates.

use super::print_no_plugins;
use crate::PluginStore;
use anyhow::{Context, Result};

/// Check versions by default; update only when a plugin or --all is supplied.
pub(super) fn run(store: &PluginStore, name: Option<&str>, all: bool, json: bool) -> Result<()> {
    if name.is_none() && !all {
        check(store, json)
    } else {
        update(store, name, all)
    }
}

/// Atomically update one plugin, or every installed plugin with `all`.
fn update(store: &PluginStore, name: Option<&str>, all: bool) -> Result<()> {
    if all {
        let results = store.update_all();
        let mut failures = Vec::new();
        let mut updated = Vec::new();
        for (name, result) in results {
            match result {
                Ok(manifest) => {
                    println!("Updated {name} to {}", manifest.version);
                    updated.push(name);
                }
                Err(error) => failures.push(format!("{name}: {error:#}")),
            }
        }
        if !failures.is_empty() {
            anyhow::bail!("Some updates failed:\n{}", failures.join("\n"));
        }
        if updated.is_empty() {
            print_no_plugins();
        }
    } else {
        let name = name.context("Provide a plugin name or use --all")?;
        let manifest = store.update(name)?;
        println!("Updated {} to {}", manifest.name, manifest.version);
    }
    Ok(())
}

/// Report installed plugins that have a newer version available.
fn check(store: &PluginStore, json: bool) -> Result<()> {
    let statuses = store.outdated()?;
    if json {
        println!("{}", serde_json::to_string_pretty(&statuses)?);
    } else if statuses.is_empty() {
        print_no_plugins();
    } else {
        for status in statuses {
            let state = match status.available_version.as_deref() {
                Some(_) if status.update_available => "update available",
                Some(_) => "current",
                // No comparable source, for example an installer package that
                // was unpacked from a temporary directory.
                None => "unknown",
            };
            println!(
                "{}\t{}\t{}\t{}",
                status.name,
                status.installed_version,
                status.available_version.as_deref().unwrap_or("unknown"),
                state
            );
        }
    }
    Ok(())
}
