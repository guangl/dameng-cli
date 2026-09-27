//! Error paths: they must stay actionable and never damage the store.

use crate::common::*;
use dameng_cli::PluginStore;
use std::fs;
use tempfile::TempDir;

#[test]
fn source_without_prebuilt_binary_is_rejected() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    fs::remove_file(source.join(format!("dm-probe{}", std::env::consts::EXE_SUFFIX))).unwrap();
    let home = temp.path().join("home");
    let output = dm(&home).arg("install").arg(&source).output().unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("prebuilt"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read_dir(home.join("plugins")).unwrap().count(), 0);
}
#[cfg(unix)]
#[test]
fn unwritable_dm_home_reports_actionable_error() {
    use std::os::unix::fs::PermissionsExt;

    let temp = TempDir::new().unwrap();
    let home = temp.path().join("readonly");
    fs::create_dir_all(&home).unwrap();
    fs::set_permissions(&home, fs::Permissions::from_mode(0o555)).unwrap();

    let error = PluginStore::new(&home).list().unwrap_err();
    assert!(error.to_string().contains("is not writable"), "{error:#}");

    fs::set_permissions(&home, fs::Permissions::from_mode(0o755)).unwrap();
}
#[test]
fn home_must_not_be_inside_plugin_source() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let store = PluginStore::new(source.join("home"));
    assert!(
        store
            .install(source.to_str().unwrap())
            .unwrap_err()
            .to_string()
            .contains("must not contain")
    );
}
#[test]
fn sqlite_store_open_error_is_actionable() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    fs::create_dir_all(home.join("store.sqlite3")).unwrap();
    let output = dm(&home).args(["list"]).output().unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("Cannot open SQLite plugin store"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn install_reports_malformed_store_query_error() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    rusqlite::Connection::open(home.join("store.sqlite3"))
        .unwrap()
        .execute_batch("CREATE TABLE installed_plugins (manifest TEXT NOT NULL)")
        .unwrap();

    let output = dm(&home).arg("install").arg(&source).output().unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("no such column"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn install_reports_sqlite_insert_failure() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    rusqlite::Connection::open(home.join("store.sqlite3"))
        .unwrap()
        .execute_batch(
            "CREATE TABLE installed_plugins (
                 name TEXT PRIMARY KEY,
                 manifest TEXT NOT NULL,
                 installed_at INTEGER NOT NULL DEFAULT (unixepoch()),
                 source TEXT,
                 revision TEXT,
                 source_ref TEXT,
                 checksum TEXT NOT NULL DEFAULT '',
                 extra TEXT NOT NULL
             ) STRICT;",
        )
        .unwrap();

    let output = dm(&home).arg("install").arg(&source).output().unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("Record installed plugin in SQLite"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn doctor_reports_invalid_plugin() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    let store = PluginStore::new(&home);
    store.install(source.to_str().unwrap()).unwrap();

    rusqlite::Connection::open(home.join("store.sqlite3"))
        .unwrap()
        .execute(
            "INSERT INTO installed_plugins (name, manifest, checksum) VALUES (?1, ?2, '')",
            rusqlite::params!["bad", manifest("bad")],
        )
        .unwrap();
    fs::create_dir_all(home.join("plugins/bad")).unwrap();
    fs::write(home.join("plugins/bad/dm-plugin.toml"), "invalid").unwrap();

    let report = store.doctor(false).unwrap();
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.contains("invalid plugin bad")),
        "{report:?}"
    );
}
#[test]
fn doctor_reports_read_dir_error() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let store = PluginStore::new(&home);
    fs::create_dir_all(&home).unwrap();
    fs::write(home.join("config"), "not a directory").unwrap();

    let error = store.doctor(false).unwrap_err();
    assert!(error.to_string().contains("Read"), "{error:#}");
}
#[test]
fn corrupt_store_fails_list_and_update_all_with_an_actionable_error() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    fs::write(home.join("store.sqlite3"), b"definitely not a database").unwrap();

    let list = dm(&home).args(["list"]).output().unwrap();
    assert!(!list.status.success());
    let stderr = String::from_utf8_lossy(&list.stderr);
    assert!(stderr.contains("file is not a database"), "{stderr}");

    let update = dm(&home).args(["update", "--all"]).output().unwrap();
    assert!(!update.status.success());
    let stderr = String::from_utf8_lossy(&update.stderr);
    assert!(stderr.contains("Some updates failed"), "{stderr}");
}
#[test]
fn doctor_refuses_to_replace_a_non_directory_plugin_path() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    let store = PluginStore::new(&home);
    store.install(source.to_str().unwrap()).unwrap();

    let previous = home.join("plugins/.install-stale/previous");
    fs::create_dir_all(previous.parent().unwrap()).unwrap();
    fs::rename(home.join("plugins/probe"), &previous).unwrap();
    fs::write(home.join("plugins/probe"), "not a directory").unwrap();

    let error = store.doctor(true).unwrap_err();
    assert!(
        format!("{error:#}").contains("Refusing to replace non-directory"),
        "{error:#}"
    );
}
