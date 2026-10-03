//! Plugin names that collide with host directories stay readable when they were
//! installed by an older release.
//!
//! The fixtures are shell scripts, so these tests run on Unix only.
#![cfg(unix)]

use crate::common::*;
use std::fs;
use tempfile::TempDir;

/// Write the store row and plugin directory an older `dm` would have left.
fn install_legacy_plugin(home: &std::path::Path, name: &str) {
    let root = home.join("plugins").join(name);
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("dm-plugin.toml"), manifest(name)).unwrap();
    fs::write(root.join(format!("dm-{name}")), "#!/bin/sh\necho legacy\n").unwrap();
    let connection = rusqlite::Connection::open(home.join("store.sqlite3")).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS installed_plugins (
                 name TEXT PRIMARY KEY,
                 manifest TEXT NOT NULL,
                 installed_at INTEGER NOT NULL DEFAULT (unixepoch()),
                 source TEXT,
                 revision TEXT,
                 source_ref TEXT,
                 checksum TEXT NOT NULL DEFAULT ''
             ) STRICT;
             CREATE TABLE IF NOT EXISTS retained_plugin_data (name TEXT PRIMARY KEY) STRICT;",
        )
        .unwrap();
    connection
        .execute(
            "INSERT OR REPLACE INTO installed_plugins (name, manifest) VALUES (?1, ?2)",
            rusqlite::params![name, manifest(name)],
        )
        .unwrap();
}

#[test]
fn plugins_named_after_host_directories_stay_usable() {
    for name in ["data", "cache", "logs", "plugins", "backups"] {
        let temp = TempDir::new().unwrap();
        let home = temp.path().join("home");
        fs::create_dir_all(&home).unwrap();
        install_legacy_plugin(&home, name);

        let list = ok(dm(&home).args(["list"]).output().unwrap());
        assert!(list.contains(name), "{name}: {list}");
        let info = ok(dm(&home).args(["info", name]).output().unwrap());
        assert!(info.contains(name), "{name}: {info}");
        ok(dm(&home)
            .args(["uninstall", name, "--purge", "--yes"])
            .output()
            .unwrap());
        assert!(!home.join("plugins").join(name).exists());
    }
}

#[test]
fn installing_a_plugin_named_after_a_host_directory_is_refused() {
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("package");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("dm-plugin.toml"), manifest("data")).unwrap();
    fs::write(source.join("dm-data"), "#!/bin/sh\n").unwrap();

    let home = temp.path().join("home");
    let failure = dm(&home)
        .args(["install", source.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!failure.status.success());
    assert!(
        stderr(&failure).contains("reserved"),
        "{}",
        stderr(&failure)
    );
}
