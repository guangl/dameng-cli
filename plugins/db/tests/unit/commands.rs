//! Tests for the add/list/remove commands, including the interactive add.

use crate::common::*;
use crate::fake_driver::FakeDriver;
use dm_plugin_db::{
    DEFAULT_DRIVER, DEFAULT_PORT, DEFAULT_USERNAME, config_path, connection_password,
    find_connection, load_connections, run_with, run_with_prompter,
};
use std::fs;
use tempfile::TempDir;

#[test]
fn add_list_and_remove_run_without_any_driver() {
    let temp = TempDir::new().unwrap();
    let mut context = context(&temp);
    let driver = FakeDriver::default();

    with_args(
        &mut context,
        &[
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
            "--driver",
            "DM8 ODBC DRIVER",
        ],
    );
    assert_eq!(run_with(&context, &driver).unwrap(), 0);
    let stored = find_connection(&context, "prod").unwrap();
    assert_eq!(stored.port, 5237);
    assert_eq!(stored.schema.as_deref(), Some("DMHR"));
    assert_eq!(
        connection_password(&context, &stored).unwrap(),
        "SYSDBA@123"
    );

    with_args(&mut context, &["list"]);
    assert_eq!(run_with(&context, &driver).unwrap(), 0);

    with_args(&mut context, &["remove", "prod"]);
    assert_eq!(run_with(&context, &driver).unwrap(), 0);
    assert!(load_connections(&context).unwrap().is_empty());
    assert_eq!(driver.opened(), 0, "these commands never open a driver");
}

#[test]
fn add_uses_the_plugin_configuration_as_defaults() {
    let temp = TempDir::new().unwrap();
    let mut context = context(&temp);
    fs::create_dir_all(&context.config_dir).unwrap();
    fs::write(
        config_path(&context),
        concat!(
            "[defaults]\nport = 5300\nusername = \"DMHR\"\n",
            "driver = \"DM8 ODBC DRIVER\"\nschema = \"DMHR\"\n",
        ),
    )
    .unwrap();

    with_args(
        &mut context,
        &["add", "prod", "--host", "10.0.0.8", "--password", "pw"],
    );
    assert_eq!(run_with(&context, &FakeDriver::default()).unwrap(), 0);
    let stored = find_connection(&context, "prod").unwrap();
    assert_eq!(stored.port, 5300);
    assert_eq!(stored.username, "DMHR");
    assert_eq!(stored.schema.as_deref(), Some("DMHR"));

    // The documented defaults apply when neither flag nor file sets a value.
    fs::write(config_path(&context), "[defaults]\nport = 5300\n").unwrap();
    with_args(
        &mut context,
        &["add", "other", "--host", "10.0.0.9", "--password", "pw"],
    );
    assert_eq!(run_with(&context, &FakeDriver::default()).unwrap(), 0);
    let stored = find_connection(&context, "other").unwrap();
    assert_eq!(stored.username, DEFAULT_USERNAME);
    assert_eq!(stored.driver, DEFAULT_DRIVER);
    assert_eq!(stored.schema, None);
}

#[test]
fn add_rejects_invalid_input() {
    let temp = TempDir::new().unwrap();
    let mut context = context(&temp);

    with_args(
        &mut context,
        &["add", "bad name", "--host", "h", "--password", "pw"],
    );
    assert!(run_with(&context, &FakeDriver::default()).is_err());

    with_args(
        &mut context,
        &["add", "prod", "--host", "  ", "--password", "pw"],
    );
    assert!(run_with(&context, &FakeDriver::default()).is_err());

    with_args(&mut context, &["add", "prod", "--host", "h"]);
    let error = run_with(&context, &FakeDriver::default()).unwrap_err();
    assert!(error.to_string().contains("--password"), "{error}");
}

#[test]
fn interactive_add_fills_in_every_missing_value() {
    let temp = TempDir::new().unwrap();
    let mut context = context(&temp);
    // Name, host, port (Enter keeps 5236), username, driver and schema.
    let script = Script::new(&["prod", "10.0.0.8", "", "DMHR", "", ""], &["SYSDBA@123"]);

    with_args(&mut context, &["add"]);
    assert_eq!(
        run_with_prompter(&context, &FakeDriver::default(), Some(&script)).unwrap(),
        0
    );

    let stored = find_connection(&context, "prod").unwrap();
    assert_eq!(stored.host, "10.0.0.8");
    assert_eq!(stored.port, DEFAULT_PORT);
    assert_eq!(stored.username, "DMHR");
    assert_eq!(stored.driver, DEFAULT_DRIVER);
    assert_eq!(stored.schema, None);
    assert_eq!(
        connection_password(&context, &stored).unwrap(),
        "SYSDBA@123"
    );
}

#[test]
fn interactive_add_reports_a_closed_terminal() {
    let temp = TempDir::new().unwrap();
    let mut context = context(&temp);

    // Every prefix of the answers makes a later prompt fail, so each interactive
    // step reports the closed terminal and nothing is saved.
    for answers in [
        &[][..],
        &["prod"][..],
        &["prod", "10.0.0.8"][..],
        &["prod", "10.0.0.8", ""][..],
        &["prod", "10.0.0.8", "", ""][..],
        &["prod", "10.0.0.8", "", "", ""][..],
    ] {
        let script = Script::new(answers, &[]);
        with_args(&mut context, &["add"]);
        let error = run_with_prompter(&context, &FakeDriver::default(), Some(&script)).unwrap_err();
        assert!(
            format!("{error:#}").contains("terminal closed"),
            "{error:#}"
        );
        assert!(load_connections(&context).unwrap().is_empty());
    }
}
