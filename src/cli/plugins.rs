//! Plugin subcommands: install, list, info, verify, uninstall and run.

use super::print_no_plugins;
use super::table;
use crate::{Config, PluginStore};
use anyhow::{Context, Result};
use std::ffi::OsString;

/// Install a prebuilt plugin from a directory or an HTTPS Git repository.
pub(super) fn install(
    store: &PluginStore,
    source: &str,
    rev: Option<&str>,
    replace: bool,
) -> Result<()> {
    let manifest = store.install_with_revision(source, rev, replace)?;
    println!("Installed {} {}", manifest.name, manifest.version);
    Ok(())
}

/// Print installed plugins as a table, or as JSON when asked.
pub(super) fn list(store: &PluginStore, json: bool) -> Result<()> {
    let plugins = store.list_info()?;
    if json {
        println!("{}", serde_json::to_string_pretty(&plugins)?);
    } else if plugins.is_empty() {
        print_no_plugins();
    } else {
        print!("{}", table::render(&plugins));
    }
    Ok(())
}

/// Print installation metadata for one plugin.
pub(super) fn info(store: &PluginStore, name: &str, json: bool) -> Result<()> {
    let plugin = store.info(name)?;
    // Plugins configure themselves inside their own directory; expose the
    // paths so users can find (and edit) the right file.
    let directories = store.plugin_directories(&plugin.manifest.name);
    let config_file = Config::path_in(&directories[0]);
    if json {
        let mut value = serde_json::to_value(&plugin)?;
        value["paths"] = serde_json::json!({
            "config": directories[0],
            "data": directories[1],
            "cache": directories[2],
            "config_file": config_file,
            "config_file_present": config_file.is_file(),
        });
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        println!("Name: {}", plugin.manifest.name);
        println!("Version: {}", plugin.manifest.version);
        println!("Source: {}", plugin.source.as_deref().unwrap_or("unknown"));
        println!(
            "Revision: {}",
            plugin.revision.as_deref().unwrap_or("unknown")
        );
        println!("SHA-256: {}", plugin.checksum);
        if !plugin.manifest.environment.is_empty() {
            println!("Environment: {}", plugin.manifest.environment.join(", "));
        }
        println!("Config dir: {}", directories[0].display());
        println!("Data dir: {}", directories[1].display());
        println!("Cache dir: {}", directories[2].display());
        println!(
            "Config file: {} ({})",
            config_file.display(),
            if config_file.is_file() {
                "present"
            } else {
                "absent"
            }
        );
    }
    Ok(())
}

/// Verify installed plugin manifests and binary checksums.
pub(super) fn verify(store: &PluginStore, name: Option<&str>) -> Result<()> {
    let names = store.verify(name)?;
    if names.is_empty() {
        print_no_plugins();
    } else {
        for name in names {
            println!("Verified {name}");
        }
    }
    Ok(())
}

/// Remove an installed plugin.
pub(super) fn uninstall(store: &PluginStore, name: &str) -> Result<()> {
    store.uninstall(name)?;
    println!("Uninstalled {name}");
    Ok(())
}

/// Run an installed plugin, forwarding every argument unchanged.
pub(super) fn run_plugin(store: &PluginStore, args: &[OsString]) -> Result<i32> {
    let name = args
        .first()
        .context("Missing plugin name")?
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Plugin name must be UTF-8"))?;
    store.run(name, &args[1..])
}
