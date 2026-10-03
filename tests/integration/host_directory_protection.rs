//! Host logs survive cleanup, including nested paths and directory aliases.
use crate::common::*;
use dameng_cli::PluginStore;
use std::fs;
use tempfile::TempDir;

#[test]
fn doctor_protects_nested_logs_in_current_legacy_and_package_layouts() {
    for directory in [
        "diagnostics/nested",
        "data/diagnostics/nested",
        "cache/diagnostics/nested",
        "config/diagnostics/nested",
        "plugins/.install-diagnostics/nested",
        "plugins/diagnostics/nested",
        "diagnostics/../diagnostics/nested",
    ] {
        let temp = TempDir::new().unwrap();
        let home = temp.path().join("home");
        fs::create_dir_all(&home).unwrap();
        write_config(&home, &format!("[log]\ndirectory = {directory:?}\n"));
        let sentinel = home.join(directory).join("keep");
        fs::create_dir_all(sentinel.parent().unwrap()).unwrap();
        fs::write(&sentinel, "host log").unwrap();
        let orphan = home.join("orphan/data");
        fs::create_dir_all(&orphan).unwrap();
        ok(dm(&home).args(["doctor", "--repair"]).output().unwrap());
        assert!(sentinel.is_file(), "{directory}");
        assert!(!orphan.exists(), "unrelated orphan should still be removed");
    }
}

#[test]
fn purge_protects_logs_inside_plugin_data_and_backups() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    for (index, directory) in [
        "probe/data/logs",
        "data/probe/logs",
        "backups/probe/logs",
        "probe/../probe/data/logs",
    ]
    .into_iter()
    .enumerate()
    {
        let home = temp.path().join(format!("home-{index}"));
        let store = PluginStore::new(&home).with_log_directory(home.join(directory));
        store.install(source.to_str().unwrap()).unwrap();
        let sentinel = home.join(directory).join("keep");
        fs::create_dir_all(sentinel.parent().unwrap()).unwrap();
        fs::write(&sentinel, "host log").unwrap();
        write_config(&home, &format!("[log]\ndirectory = {directory:?}\n"));
        if index % 2 == 1 {
            ok(dm(&home)
                .args(["uninstall", "probe", "--yes"])
                .output()
                .unwrap());
            assert!(store.has_retained_data("probe").unwrap());
        }
        let output = ok(dm(&home)
            .args(["uninstall", "probe", "--purge", "--yes"])
            .output()
            .unwrap());
        assert!(output.contains("与宿主日志重叠的目录已保留"));
        assert!(sentinel.is_file(), "{directory}");
        assert!(!home.join("plugins/probe").exists());
        assert!(!store.has_retained_data("probe").unwrap());
        store.doctor(true).unwrap();
        assert!(sentinel.is_file(), "{directory}: repair after purge");
    }
}

#[test]
fn uninstall_refuses_to_remove_a_package_holding_host_logs() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let store = PluginStore::new(&home);
    let source = fixture(temp.path());
    store.install(source.to_str().unwrap()).unwrap();
    write_config(&home, "[log]\ndirectory = \"plugins/probe/logs\"\n");
    let sentinel = home.join("plugins/probe/logs/keep");
    fs::create_dir_all(sentinel.parent().unwrap()).unwrap();
    fs::write(&sentinel, "host log").unwrap();
    let output = dm(&home)
        .args(["uninstall", "probe", "--purge", "--yes"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(stderr(&output).contains("move the host log directory"));
    assert!(sentinel.is_file());
    store.info("probe").unwrap();
}

#[cfg(unix)]
#[test]
fn cleanup_resolves_log_directory_symlinks() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let actual = home.join("diagnostics/nested");
    fs::create_dir_all(&actual).unwrap();
    std::os::unix::fs::symlink(home.join("diagnostics"), home.join("alias")).unwrap();
    let store = PluginStore::new(&home).with_log_directory(home.join("alias/nested"));
    let sentinel = actual.join("keep");
    fs::write(&sentinel, "host log").unwrap();
    store.doctor(true).unwrap();
    assert!(sentinel.is_file());
    // Missing leaf components must still resolve through the existing alias.
    let store = PluginStore::new(&home).with_log_directory(home.join("alias/future/logs"));
    store.doctor(true).unwrap();
    assert!(sentinel.is_file());
    // Purging a retained plugin whose root aliases the log tree is protected.
    std::os::unix::fs::symlink(home.join("diagnostics"), home.join("probe")).unwrap();
    assert!(!store.removal_paths("probe").contains(&home.join("probe")));
}

#[cfg(unix)]
#[test]
fn doctor_preserves_ancestors_of_log_symlinks_pointing_outside_home() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let external = temp.path().join("external");
    fs::create_dir_all(home.join("diagnostics")).unwrap();
    fs::create_dir_all(&external).unwrap();
    std::os::unix::fs::symlink(&external, home.join("diagnostics/link")).unwrap();
    let logs = home.join("diagnostics/link/logs");
    fs::create_dir_all(&logs).unwrap();
    fs::write(logs.join("keep"), "host log").unwrap();
    let store = PluginStore::new(&home).with_log_directory(&logs);
    store.doctor(true).unwrap();
    assert!(logs.join("keep").is_file());
    assert!(home.join("diagnostics/link").is_symlink());
}
