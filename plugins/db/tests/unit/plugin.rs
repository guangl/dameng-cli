//! Tests for the plugin entry point, its error hints and the public constants.

use crate::common::*;
use dm_plugin_db::driver::PendingFactory;
use dm_plugin_db::{DEFAULT_DRIVER, DEFAULT_PORT, DEFAULT_USERNAME, db_hint, run_with};
use tempfile::TempDir;

#[test]
fn the_pending_driver_reports_that_it_is_not_implemented() {
    let temp = TempDir::new().unwrap();
    let mut context = context(&temp);
    stored_connection(&context, "prod", None);

    // test and exec reach the driver and report the deferred implementation.
    for args in [vec!["test", "prod"], vec!["exec", "prod", "SELECT 1"]] {
        with_args(&mut context, &args);
        let error = run_with(&context, &PendingFactory).unwrap_err();
        assert!(
            format!("{error:#}").contains("not implemented"),
            "{error:#}"
        );
        assert!(db_hint(&error).contains("尚未接入"), "{}", db_hint(&error));
    }

    // The commands that never open a driver keep working.
    with_args(&mut context, &["list"]);
    assert_eq!(run_with(&context, &PendingFactory).unwrap(), 0);
}

#[test]
fn hints_cover_every_reported_failure_class() {
    let hint = |message: &str| db_hint(&anyhow::anyhow!(message.to_owned()));
    assert!(hint("Database connection 'prod' is not configured").contains("dm db add"));
    assert!(hint("The database driver is not implemented yet").contains("尚未接入"));
    assert!(hint("The driver name must not be empty").contains("必填项"));
    assert!(hint("no such table: connections").contains("connections.sqlite3"));
    assert!(
        hint("table connections has no column named host: SQL logic error")
            .contains("connections.sqlite3")
    );
    assert!(hint("something else entirely").contains("dm db --help"));
}

#[test]
fn the_plugin_entry_point_reports_errors() {
    let temp = TempDir::new().unwrap();
    let mut context = context(&temp);
    with_args(&mut context, &["remove", "missing"]);
    let error = dm_plugin_db::run_cli(&context).unwrap_err();
    assert!(format!("{error:#}").contains("not configured"), "{error:#}");
}

#[test]
fn default_constants_match_the_documented_values() {
    assert_eq!(DEFAULT_PORT, 5236);
    assert_eq!(DEFAULT_USERNAME, "SYSDBA");
    assert_eq!(DEFAULT_DRIVER, "DM8 ODBC DRIVER");
}
