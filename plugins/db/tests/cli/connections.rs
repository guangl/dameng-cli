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
    assert!(list.contains("prod\tSYSDBA@10.0.0.8:5237\tDMHR"), "{list}");
    assert!(list.contains("DM8 ODBC DRIVER"), "{list}");
    assert!(!list.contains("SYSDBA@123"), "{list}");

    let remove = ok(db(&home).args(["remove", "prod"]).output().unwrap());
    assert!(
        remove.contains("Removed database connection prod"),
        "{remove}"
    );
    assert!(ok(db(&home).args(["list"]).output().unwrap()).is_empty());
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
    assert!(list.contains("DMHR@10.0.0.8:5300\tDMHR"), "{list}");

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
    assert!(list.contains("SYSDBA@10.0.0.9:5236\t-"), "{list}");
}
