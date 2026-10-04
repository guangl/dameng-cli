//! Log protection during interrupted package recovery.
use crate::common::*;
use dameng_cli::PluginStore;
use std::fs;
use tempfile::TempDir;

#[test]
fn interrupted_transaction_cannot_replace_a_package_holding_logs() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let source = fixture(temp.path());
    let store = PluginStore::new(&home).with_log_directory(home.join("plugins/probe/logs"));
    store.install(source.to_str().unwrap()).unwrap();
    let previous = home.join("plugins/.remove-interrupted/package");
    fs::create_dir_all(previous.parent().unwrap()).unwrap();
    fs::rename(home.join("plugins/probe"), &previous).unwrap();
    fs::create_dir_all(home.join("plugins/probe/logs")).unwrap();
    fs::write(home.join("plugins/probe/logs/keep"), "host log").unwrap();
    let report = store.doctor(true).unwrap();
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.contains("stale transaction"))
    );
    assert!(home.join("plugins/probe/logs/keep").is_file());
    assert!(previous.join("dm-plugin.toml").is_file());
    store.info("probe").unwrap();
}
