use crate::{common::*, fake_driver::FakeDriver};
use dm_plugin_db::{
    connection_password, find_connection, load_connections, run_with, run_with_prompter,
};
use tempfile::TempDir;
#[test]
fn edit_preserves_password_and_changes_only_requested_fields() {
    let temp = TempDir::new().unwrap();
    let mut context = context(&temp);
    stored_connection(&context, "prod", Some("DMHR"));
    with_args(&mut context, &["edit", "prod", "--port", "5300"]);
    run_with(&context, &FakeDriver::default()).unwrap();
    let entry = find_connection(&context, "prod").unwrap();
    assert_eq!(entry.port, 5300);
    assert_eq!(entry.schema.as_deref(), Some("DMHR"));
    assert_eq!(connection_password(&context, &entry).unwrap(), "SYSDBA@123");
    with_args(
        &mut context,
        &["edit", "prod", "--clear-schema", "--password", "new"],
    );
    run_with(&context, &FakeDriver::default()).unwrap();
    let entry = find_connection(&context, "prod").unwrap();
    assert_eq!(entry.schema, None);
    assert_eq!(connection_password(&context, &entry).unwrap(), "new");
    with_args(&mut context, &["edit", "prod", "--schema", "NEW"]);
    run_with(&context, &FakeDriver::default()).unwrap();
    assert_eq!(
        find_connection(&context, "prod").unwrap().schema.as_deref(),
        Some("NEW")
    );
    with_args(&mut context, &["edit", "missing"]);
    assert!(run_with(&context, &FakeDriver::default()).is_err());
}
#[test]
fn duplicate_add_requires_replace_and_confirmation_cancels_without_writes() {
    let temp = TempDir::new().unwrap();
    let mut context = context(&temp);
    stored_connection(&context, "prod", None);
    with_args(
        &mut context,
        &["add", "prod", "--host", "new", "--password", "new"],
    );
    assert!(
        run_with(&context, &FakeDriver::default())
            .unwrap_err()
            .to_string()
            .contains("edit")
    );
    with_args(
        &mut context,
        &[
            "add",
            "prod",
            "--replace",
            "--host",
            "new",
            "--password",
            "new",
        ],
    );
    run_with(&context, &FakeDriver::default()).unwrap();
    assert_eq!(find_connection(&context, "prod").unwrap().host, "new");
    with_args(
        &mut context,
        &["add", "other", "--host", "h", "--password", "pw"],
    );
    let script = Script::new(&["", "", "", "", "n"], &[]);
    assert!(run_with_prompter(&context, &FakeDriver::default(), Some(&script)).is_err());
    assert_eq!(load_connections(&context).unwrap().len(), 1);
    with_args(&mut context, &["remove", "prod"]);
    assert!(run_with(&context, &FakeDriver::default()).is_err());
}
#[test]
fn interactive_edit_keeps_old_values_and_can_change_schema() {
    let temp = TempDir::new().unwrap();
    let mut context = context(&temp);
    stored_connection(&context, "prod", Some("DMHR"));
    with_args(&mut context, &["edit", "prod"]);
    let script = Script::new(&["", "", "", "bad", "", "", "y"], &[]);
    run_with_prompter(&context, &FakeDriver::default(), Some(&script)).unwrap();
    assert_eq!(
        find_connection(&context, "prod").unwrap().schema.as_deref(),
        Some("DMHR")
    );
    let script = Script::new(&["h", "user", "driver", "6000", "NEW", "y"], &[]);
    run_with_prompter(&context, &FakeDriver::default(), Some(&script)).unwrap();
    assert_eq!(
        find_connection(&context, "prod").unwrap().schema.as_deref(),
        Some("NEW")
    );
}
#[test]
fn plugin_config_doctor_and_completion_work_without_driver() {
    let temp = TempDir::new().unwrap();
    let mut context = context(&temp);
    for args in [
        &["config", "path"][..],
        &["config", "init"],
        &["config", "show"],
        &["config", "show", "--json"],
    ] {
        with_args(&mut context, args);
        run_with(&context, &FakeDriver::default()).unwrap();
    }
    for args in [&["doctor"][..], &["doctor", "--json"]] {
        with_args(&mut context, args);
        assert_eq!(run_with(&context, &FakeDriver::default()).unwrap(), 1);
    }
    for args in [
        &["__complete", ""][..],
        &["__complete", "edit", ""],
        &["__complete", "add", "--"],
    ] {
        with_args(&mut context, args);
        run_with(&context, &FakeDriver::default()).unwrap();
    }
}
