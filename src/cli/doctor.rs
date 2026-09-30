//! The `dm doctor` subcommand.

use crate::PluginStore;
use anyhow::Result;

/// Diagnose the plugin store and optionally repair recoverable issues.
pub(super) fn doctor(store: &PluginStore, repair: bool, json: bool) -> Result<()> {
    let report = store.doctor(repair)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else if report.issues.is_empty() {
        println!("插件存储正常");
    } else {
        for issue in &report.issues {
            println!("问题： {issue}");
        }
        for repair in &report.repairs {
            println!("已修复： {repair}");
        }
        if !repair && report.repairs.is_empty() {
            println!("运行 dm doctor --repair 修复可恢复问题");
        }
    }
    Ok(())
}
