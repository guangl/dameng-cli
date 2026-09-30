//! End-to-end tests for add, list and remove.

use crate::common::*;
use std::fs;
use tempfile::TempDir;

#[test]
fn add_list_and_remove_connections() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();

    let add = ok(db(&home)
        .args([
            "add",
            "prod",
            "--host",
            "10.0.0.8",
            "--port",
            "5237",
            "--username",
            "SYSDBA",
            "--password",
            "SYSDBA@123",
            "--schema",
            "DMHR",
        ])
        .output()
        .unwrap());
    assert!(add.contains("Saved database connection prod"), "{add}");
    assert!(
        !add.contains("SYSDBA@123"),
        "the password must not be echoed"
    );

    let list = ok(db(&home).args(["list"]).output().unwrap());
    for needle in [
        "Name",
        "Host",
        "Port",
        "User",
        "Schema",
        "Driver",
        "prod",
        "10.0.0.8",
        "5237",
        "SYSDBA",
        "DMHR",
        "DM8 ODBC DRIVER",
    ] {
        assert!(list.contains(needle), "missing {needle:?} in:\n{list}");
    }
    assert!(!list.contains("SYSDBA@123"), "{list}");
    assert!(list.ends_with('\n'), "the table must end with a newline");

    let remove = ok(db(&home).args(["remove", "prod"]).output().unwrap());
    assert!(
        remove.contains("Removed database connection prod"),
        "{remove}"
    );
    let empty = ok(db(&home).args(["list"]).output().unwrap());
    assert!(empty.contains("dm db add <name>"), "{empty}");
}

#[test]
fn add_uses_the_plugin_configuration_defaults() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    write_plugin_config(
        &home,
        concat!(
            "[defaults]\nport = 5300\nusername = \"DMHR\"\n",
            "driver = \"DM8 ODBC DRIVER\"\nschema = \"DMHR\"\n",
        ),
    );

    ok(db(&home)
        .args(["add", "prod", "--host", "10.0.0.8", "--password", "pw"])
        .output()
        .unwrap());
    let list = ok(db(&home).args(["list"]).output().unwrap());
    for needle in ["DMHR", "10.0.0.8", "5300"] {
        assert!(list.contains(needle), "missing {needle:?} in:\n{list}");
    }

    // An explicit flag wins over the plugin configuration.
    ok(db(&home)
        .args([
            "add",
            "explicit",
            "--host",
            "10.0.0.9",
            "--port",
            "5236",
            "--username",
            "SYSDBA",
            "--password",
            "pw",
            "--schema",
            "",
        ])
        .output()
        .unwrap());
    let list = ok(db(&home).args(["list"]).output().unwrap());
    for needle in ["SYSDBA", "10.0.0.9", "5236"] {
        assert!(list.contains(needle), "missing {needle:?} in:\n{list}");
    }
    // No schema configured for the connection, so the cell shows a placeholder.
    assert!(list.contains("-"), "{list}");
}

#[test]
fn list_json_reports_connections_without_passwords() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();

    let empty = ok(db(&home).args(["list", "--json"]).output().unwrap());
    assert_eq!(empty.trim(), "[]");

    ok(db(&home)
        .args([
            "add",
            "prod",
            "--host",
            "10.0.0.8",
            "--port",
            "5237",
            "--username",
            "SYSDBA",
            "--password",
            "SYSDBA@123",
            "--schema",
            "DMHR",
        ])
        .output()
        .unwrap());

    let output = ok(db(&home).args(["list", "--json"]).output().unwrap());
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let entries = parsed.as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["name"], "prod");
    assert_eq!(entries[0]["host"], "10.0.0.8");
    assert_eq!(entries[0]["port"], 5237);
    assert_eq!(entries[0]["username"], "SYSDBA");
    assert_eq!(entries[0]["schema"], "DMHR");
    assert_eq!(entries[0]["driver"], "DM8 ODBC DRIVER");
    assert!(!output.contains("SYSDBA@123"), "{output}");
}
