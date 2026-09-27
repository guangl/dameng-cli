//! End-to-end tests for the plugin configuration and the store failures.

use crate::common::*;
use std::fs;
use tempfile::TempDir;

#[test]
fn invalid_plugin_configuration_is_reported() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    write_plugin_config(&home, "[connect]\ntimeout = 0\n");

    // Commands that do not need the values still work.
    assert!(db(&home).args(["list"]).output().unwrap().status.success());

    let stderr = failure(
        db(&home)
            .args(["add", "prod", "--host", "10.0.0.8", "--password", "pw"])
            .output()
            .unwrap(),
    );
    assert!(stderr.contains("connect.timeout"), "{stderr}");
    assert!(stderr.contains("config.toml"), "{stderr}");
}

#[test]
fn malformed_store_is_reported() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let data_dir = home.join("data/db");
    fs::create_dir_all(&data_dir).unwrap();
    let database = data_dir.join("connections.sqlite3");
    let connection = rusqlite::Connection::open(&database).unwrap();
    connection
        .execute_batch("CREATE TABLE connections (name TEXT PRIMARY KEY)")
        .unwrap();
    drop(connection);

    let stderr = failure(
        db(&home)
            .args(["add", "prod", "--host", "10.0.0.8", "--password", "pw"])
            .output()
            .unwrap(),
    );
    assert!(stderr.contains("dm db:"), "{stderr}");
    assert!(stderr.contains("connections.sqlite3"), "{stderr}");
}
