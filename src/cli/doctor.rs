//! The `dm doctor` subcommand.

use crate::PluginStore;
use anyhow::Result;

/// Diagnose the plugin store and optionally repair recoverable issues.
pub(super) fn doctor(store: &PluginStore, repair: bool, json: bool) -> Result<()> {
    let report = store.doctor(repair)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else if report.issues.is_empty() {
        println!("Plugin store is healthy");
    } else {
        for issue in &report.issues {
            println!("Issue: {issue}");
        }
        for repair in &report.repairs {
            println!("Repaired: {repair}");
        }
        if !repair && report.repairs.is_empty() {
            println!("Run dm doctor --repair to repair recoverable issues");
        }
    }
    Ok(())
}
