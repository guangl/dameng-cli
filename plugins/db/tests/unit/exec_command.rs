//! Tests for "dm db exec" against the scripted driver.

use crate::common::*;
use crate::fake_driver::{DriverState, FakeDriver};
use dm_plugin_db::{QueryResult, run_with};
use std::fs;
use tempfile::TempDir;

#[test]
fn exec_prints_rows_and_affected_counts() {
    let temp = TempDir::new().unwrap();
    let mut context = context(&temp);
    stored_connection(&context, "prod", None);

    let driver = FakeDriver::new(DriverState {
        rows: Some(QueryResult {
            columns: vec!["ID".to_owned()],
            rows: vec![vec![Some("1".to_owned())]],
        }),
        ..DriverState::default()
    });
    with_args(&mut context, &["exec", "prod", "SELECT ID FROM T"]);
    assert_eq!(run_with(&context, &driver).unwrap(), 0);
    assert_eq!(driver.statements(), vec!["SELECT ID FROM T".to_owned()]);

    let driver = FakeDriver::new(DriverState {
        affected: 2,
        ..DriverState::default()
    });
    with_args(&mut context, &["exec", "prod", "UPDATE T SET A = 1"]);
    assert_eq!(run_with(&context, &driver).unwrap(), 0);
}

#[test]
fn exec_reads_sql_from_a_file_and_reports_failures() {
    let temp = TempDir::new().unwrap();
    let mut context = context(&temp);
    stored_connection(&context, "prod", None);
    let file = temp.path().join("script.sql");
    fs::write(&file, "SELECT 1;\n").unwrap();

    let driver = FakeDriver::new(DriverState {
        affected: 1,
        ..DriverState::default()
    });
    with_args(
        &mut context,
        &["exec", "prod", "--file", file.to_str().unwrap()],
    );
    assert_eq!(run_with(&context, &driver).unwrap(), 0);
    assert_eq!(driver.statements(), vec!["SELECT 1;".to_owned()]);

    // Empty SQL fails before the store or the driver is touched.
    with_args(&mut context, &["exec", "prod", "   "]);
    let error = run_with(&context, &FakeDriver::default()).unwrap_err();
    assert!(
        format!("{error:#}").contains("SQL must not be empty"),
        "{error:#}"
    );

    // Unknown connections are reported even when the SQL is fine.
    with_args(&mut context, &["exec", "missing", "SELECT 1"]);
    assert!(run_with(&context, &FakeDriver::default()).is_err());

    // Statement failures surface as errors.
    let driver = FakeDriver::new(DriverState {
        run_error: Some("Execute SQL failed".to_owned()),
        ..DriverState::default()
    });
    with_args(&mut context, &["exec", "prod", "SELECT 1"]);
    assert!(run_with(&context, &driver).is_err());
}
