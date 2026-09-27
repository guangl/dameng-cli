use std::fs;

use tempfile::TempDir;

use crate::common::*;

#[test]
fn add_list_and_remove_servers() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();

    let add = ok(ssh(&home)
        .args([
            "add",
            "prod",
            "--host",
            "10.0.0.8",
            "--port",
            "2222",
            "--username",
            "root",
            "--password",
            "p@ssw0rd",
        ])
        .output()
        .unwrap());
    assert!(add.contains("Saved SSH server prod"), "{add}");

    let list = ok(ssh(&home).args(["list"]).output().unwrap());
    assert!(list.contains("prod"), "{list}");
    assert!(list.contains("10.0.0.8"), "{list}");
    assert!(list.contains("2222"), "{list}");

    let remove = ok(ssh(&home).args(["remove", "prod"]).output().unwrap());
    assert!(remove.contains("Removed SSH server prod"), "{remove}");
    assert!(ok(ssh(&home).args(["list"]).output().unwrap()).is_empty());
}

#[test]
fn add_reports_error_for_malformed_store() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let data_dir = home.join("data/ssh");
    fs::create_dir_all(&data_dir).unwrap();
    let database = data_dir.join("servers.sqlite3");
    let connection = rusqlite::Connection::open(&database).unwrap();
    connection
        .execute_batch("CREATE TABLE servers (name TEXT PRIMARY KEY)")
        .unwrap();
    drop(connection);

    let output = ssh(&home)
        .args([
            "add",
            "bad",
            "--host",
            "10.0.0.1",
            "--username",
            "root",
            "--password",
            "p@ssw0rd",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("dm ssh:"));
}
