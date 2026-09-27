//! `dm doctor`: checksums, interrupted transactions and orphaned directories.

use crate::common::*;
use dameng_cli::PluginStore;
use std::fs;
use tempfile::TempDir;

#[test]
fn doctor_repairs_missing_checksums_and_stale_transactions() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    let store = PluginStore::new(&home);
    store.install(source.to_str().unwrap()).unwrap();
    rusqlite::Connection::open(home.join("store.sqlite3"))
        .unwrap()
        .execute(
            "UPDATE installed_plugins SET checksum = '' WHERE name = 'probe'",
            [],
        )
        .unwrap();
    let stale = home.join("plugins/.install-stale");
    fs::create_dir(&stale).unwrap();

    let report = store.doctor(false).unwrap();
    assert_eq!(report.issues.len(), 2);
    let repaired = store.doctor(true).unwrap();
    assert_eq!(repaired.repairs.len(), 2);
    assert!(!stale.exists());
    store.verify(Some("probe")).unwrap();
}
#[test]
fn doctor_restores_an_interrupted_removal() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    let store = PluginStore::new(&home);
    store.install(source.to_str().unwrap()).unwrap();
    let interrupted = home.join("plugins/.remove-interrupted");
    fs::create_dir(&interrupted).unwrap();
    fs::rename(home.join("plugins/probe"), interrupted.join("package")).unwrap();

    let report = store.doctor(true).unwrap();
    assert!(
        report
            .repairs
            .iter()
            .any(|repair| repair.contains("restored interrupted"))
    );
    store.verify(Some("probe")).unwrap();
}
#[test]
fn doctor_cleans_orphaned_per_plugin_directories() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let store = PluginStore::new(&home);
    for directory in ["config", "data", "cache"] {
        let orphan = home.join(directory).join("orphan");
        fs::create_dir_all(&orphan).unwrap();
        fs::write(orphan.join("leftover"), "leftover").unwrap();
    }

    let report = store.doctor(false).unwrap();
    assert_eq!(report.issues.len(), 3);
    let repaired = store.doctor(true).unwrap();
    assert!(
        repaired
            .repairs
            .iter()
            .any(|repair| repair.contains("config/orphan"))
    );
    for directory in ["config", "data", "cache"] {
        assert!(!home.join(directory).join("orphan").exists());
    }
}
#[cfg(unix)]
#[test]
fn doctor_reports_and_repairs_database_and_disk_mismatches() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    let store = PluginStore::new(&home);
    store.install(source.to_str().unwrap()).unwrap();

    fs::remove_dir_all(home.join("plugins/probe")).unwrap();
    let report = store.doctor(false).unwrap();
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.contains("database entry without plugin directory")),
        "{report:?}"
    );
    store.doctor(true).unwrap();
    assert!(store.list().unwrap().is_empty());

    let disk = home.join("plugins/disk");
    fs::create_dir_all(&disk).unwrap();
    fs::write(disk.join("dm-plugin.toml"), manifest("disk")).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::write(disk.join("dm-disk"), "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(disk.join("dm-disk"), fs::Permissions::from_mode(0o755)).unwrap();
    }
    let report = store.doctor(false).unwrap();
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.contains("plugin directory without database entry")),
        "{report:?}"
    );
    store.doctor(true).unwrap();
    assert_eq!(store.info("disk").unwrap().manifest.name, "disk");
}
#[test]
fn doctor_skips_files_and_installed_names() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    let store = PluginStore::new(&home);
    store.install(source.to_str().unwrap()).unwrap();

    fs::create_dir_all(home.join("config")).unwrap();
    fs::write(home.join("config/somefile"), "file").unwrap();
    fs::create_dir_all(home.join("config/probe")).unwrap();

    let report = store.doctor(false).unwrap();
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.contains("orphaned config")),
        "{report:?}"
    );
}
#[test]
fn doctor_restores_interrupted_transaction_over_existing_directory() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    let store = PluginStore::new(&home);
    store.install(source.to_str().unwrap()).unwrap();

    // A stale transaction holds the good copy while a different version is already
    // published at the destination path, so repair has to replace it.
    let previous = home.join("plugins/.install-stale/previous");
    fs::create_dir_all(previous.parent().unwrap()).unwrap();
    fs::rename(home.join("plugins/probe"), &previous).unwrap();
    fs::create_dir_all(home.join("plugins/probe")).unwrap();
    fs::write(
        home.join("plugins/probe/dm-plugin.toml"),
        manifest("probe").replace("test", "stale"),
    )
    .unwrap();

    let report = store.doctor(true).unwrap();
    assert!(
        report
            .repairs
            .iter()
            .any(|repair| repair.contains("restored interrupted transaction for probe")),
        "{report:?}"
    );
    store.verify(Some("probe")).unwrap();
    assert!(!home.join("plugins/.install-stale").exists());
}
