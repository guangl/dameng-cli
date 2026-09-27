//! Tests for "dm db test" against the scripted driver.

use crate::common::*;
use crate::fake_driver::{DriverState, FakeDriver};
use dm_plugin_db::{config_path, db_hint, run_with, upsert_connection};
use std::{fs, time::Duration};
use tempfile::TempDir;

#[test]
fn test_command_connects_and_runs_the_probe() {
    let temp = TempDir::new().unwrap();
    let mut context = context(&temp);
    stored_connection(&context, "prod", Some("DMHR"));
    fs::create_dir_all(&context.config_dir).unwrap();
    fs::write(
        config_path(&context),
        "[connect]\ntimeout = 7\nprobe = \"SELECT 1 FROM DUAL\"\n",
    )
    .unwrap();

    let driver = FakeDriver::default();
    with_args(&mut context, &["test", "prod"]);
    assert_eq!(run_with(&context, &driver).unwrap(), 0);
    assert_eq!(driver.opened(), 1);
    assert_eq!(driver.statements(), vec!["SELECT 1 FROM DUAL".to_owned()]);
    let specs = driver.specs();
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].login_timeout, Duration::from_secs(7));
    assert!(specs[0].connection_string.contains("PWD=SYSDBA@123;"));
    assert!(specs[0].connection_string.contains("Schema=DMHR;"));
}

#[test]
fn test_command_uses_the_default_probe_without_configuration() {
    let temp = TempDir::new().unwrap();
    let mut context = context(&temp);
    stored_connection(&context, "prod", None);
    let driver = FakeDriver::default();
    with_args(&mut context, &["test", "prod"]);
    assert_eq!(run_with(&context, &driver).unwrap(), 0);
    assert_eq!(driver.statements(), vec!["SELECT 1".to_owned()]);
    let specs = driver.specs();
    assert_eq!(specs[0].login_timeout, Duration::from_secs(10));
    assert!(!specs[0].connection_string.contains("Schema="));
}

#[test]
fn test_command_reports_missing_connections_and_driver_failures() {
    let temp = TempDir::new().unwrap();
    let mut context = context(&temp);

    with_args(&mut context, &["test", "missing"]);
    let error = run_with(&context, &FakeDriver::default()).unwrap_err();
    assert!(format!("{error:#}").contains("not configured"), "{error:#}");
    assert!(db_hint(&error).contains("dm db add"));

    // A stored connection without a password cannot connect.
    upsert_connection(&context, &sample("nopass")).unwrap();
    with_args(&mut context, &["test", "nopass"]);
    let error = run_with(&context, &FakeDriver::default()).unwrap_err();
    assert!(format!("{error:#}").contains("password"), "{error:#}");

    // Driver-level failures surface unchanged.
    stored_connection(&context, "prod", None);
    let driver = FakeDriver::new(DriverState {
        connect_error: Some("Connect through the driver failed".to_owned()),
        ..DriverState::default()
    });
    with_args(&mut context, &["test", "prod"]);
    let error = run_with(&context, &driver).unwrap_err();
    assert!(
        format!("{error:#}").contains("Connect through the driver failed"),
        "{error:#}"
    );
}
