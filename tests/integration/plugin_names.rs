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
    let executable = root.join(format!("dm-{name}"));
    fs::write(&executable, "#!/bin/sh\nprintf '%s\\n' \"$DM_PLUGIN_CONFIG_DIR\" \"$DM_PLUGIN_DATA_DIR\" \"$DM_PLUGIN_CACHE_DIR\"\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(executable, fs::Permissions::from_mode(0o755)).unwrap();
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
        let mut protected = Vec::new();
        for directory in ["config", "data", "cache", "plugins", "backups", "logs"] {
            let sentinel = home.join(directory).join("other").join("keep");
            fs::create_dir_all(sentinel.parent().unwrap()).unwrap();
            fs::write(&sentinel, "other plugin or host data").unwrap();
            protected.push(sentinel);
        }
        for kind in ["config", "data", "cache"] {
            let directory = home.join(kind).join(name);
            fs::create_dir_all(&directory).unwrap();
            fs::write(directory.join("keep"), "legacy plugin data").unwrap();
        }

        let output = ok(dm(&home).arg(name).output().unwrap());
        for kind in ["config", "data", "cache"] {
            let directory = home.join(kind).join(name);
            assert!(
                output.contains(directory.to_str().unwrap()),
                "{name}: {output}"
            );
            assert_eq!(
                fs::read_to_string(directory.join("keep")).unwrap(),
                "legacy plugin data"
            );
        }

        let list = ok(dm(&home).args(["list"]).output().unwrap());
        assert!(list.contains(name), "{name}: {list}");
        let info = ok(dm(&home).args(["info", name]).output().unwrap());
        assert!(info.contains(name), "{name}: {info}");
        ok(dm(&home)
            .args(["uninstall", name, "--purge", "--yes"])
            .output()
            .unwrap());
        assert!(!home.join("plugins").join(name).exists());
        for sentinel in protected {
            assert!(sentinel.is_file(), "{name}: lost {}", sentinel.display());
        }
        for kind in ["config", "data", "cache"] {
            assert!(!home.join(kind).join(name).exists());
        }
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
