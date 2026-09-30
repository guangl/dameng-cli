//! End-to-end tests for the errors reported before or at the driver.

use crate::common::*;
use std::fs;
use tempfile::TempDir;

#[test]
fn missing_arguments_and_connections_fail_with_hints() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();

    // Without a terminal the plugin refuses to guess the password.
    let stderr = failure(
        db(&home)
            .args(["add", "prod", "--host", "10.0.0.8"])
            .output()
            .unwrap(),
    );
    assert!(stderr.contains("--password"), "{stderr}");
    assert!(stderr.contains("提示"), "{stderr}");

    let empty = ok(db(&home).args(["list"]).output().unwrap());
    assert!(empty.contains("dm db add <name>"), "{empty}");

    let stderr = failure(db(&home).args(["test", "missing"]).output().unwrap());
    assert!(stderr.contains("not configured"), "{stderr}");
    assert!(stderr.contains("dm db add"), "{stderr}");

    let stderr = failure(db(&home).args(["remove", "missing"]).output().unwrap());
    assert!(stderr.contains("not configured"), "{stderr}");
}

#[test]
fn test_reports_the_deferred_driver() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    ok(db(&home)
        .args(["add", "prod", "--host", "10.0.0.8", "--password", "pw"])
        .output()
        .unwrap());

    let stderr = failure(db(&home).args(["test", "prod"]).output().unwrap());
    assert!(stderr.contains("driver is not implemented"), "{stderr}");
    assert!(stderr.contains("尚未接入"), "{stderr}");
}
