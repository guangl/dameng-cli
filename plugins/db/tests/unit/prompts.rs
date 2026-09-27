//! Tests for the interactive prompts and their non-interactive fallbacks.

use crate::common::*;
use dm_plugin_db::{
    DEFAULT_DRIVER, DEFAULT_PORT, DEFAULT_USERNAME, Prompter, resolve_optional, resolve_password,
    resolve_port, resolve_required, resolve_with_default,
};

#[test]
fn prompts_supply_required_and_defaulted_values() {
    let script = Script::new(&["prod", "", "5237", "DMHR", "", ""], &["SYSDBA@123"]);
    let prompter = Some(&script as &dyn Prompter);

    assert_eq!(
        resolve_required(None, "Name: ", "name is required", prompter).unwrap(),
        "prod"
    );
    assert_eq!(resolve_port(None, None, prompter).unwrap(), DEFAULT_PORT);
    assert_eq!(resolve_port(None, None, prompter).unwrap(), 5237);
    assert_eq!(resolve_port(Some(6000), None, prompter).unwrap(), 6000);
    assert_eq!(
        resolve_with_default(None, None, DEFAULT_USERNAME, "Username: ", prompter).unwrap(),
        "DMHR"
    );
    assert_eq!(
        resolve_with_default(None, None, DEFAULT_DRIVER, "Driver: ", prompter).unwrap(),
        DEFAULT_DRIVER
    );
    assert_eq!(resolve_optional(None, "Schema: ", prompter).unwrap(), None);
    assert_eq!(resolve_password(None, prompter).unwrap(), "SYSDBA@123");
}

#[test]
fn prompts_fall_back_without_a_terminal() {
    assert_eq!(
        resolve_required(Some("prod".to_owned()), "Name: ", "missing", None).unwrap(),
        "prod"
    );
    assert!(resolve_required(None, "Name: ", "name is required", None).is_err());
    assert_eq!(resolve_port(None, None, None).unwrap(), DEFAULT_PORT);
    assert_eq!(resolve_port(None, Some(5300), None).unwrap(), 5300);
    assert_eq!(
        resolve_with_default(None, Some("DMHR".to_owned()), DEFAULT_USERNAME, "U: ", None).unwrap(),
        "DMHR"
    );
    assert_eq!(
        resolve_with_default(None, None, DEFAULT_USERNAME, "U: ", None).unwrap(),
        DEFAULT_USERNAME
    );
    assert!(
        resolve_with_default(Some("  ".to_owned()), None, DEFAULT_USERNAME, "U: ", None).is_err()
    );
    assert_eq!(resolve_optional(None, "Schema: ", None).unwrap(), None);
    assert_eq!(
        resolve_optional(Some(" DMHR ".to_owned()), "Schema: ", None)
            .unwrap()
            .as_deref(),
        Some("DMHR")
    );
    assert_eq!(
        resolve_optional(Some("  ".to_owned()), "Schema: ", None).unwrap(),
        None
    );
    assert_eq!(resolve_password(Some("pw".to_owned()), None).unwrap(), "pw");
    assert!(resolve_password(Some(String::new()), None).is_err());
    let error = resolve_password(None, None).unwrap_err();
    assert!(error.to_string().contains("--password"), "{error}");
}

#[test]
fn prompts_report_invalid_answers() {
    let script = Script::new(&["not-a-port"], &[""]);
    let prompter = Some(&script as &dyn Prompter);
    let error = resolve_port(None, None, prompter).unwrap_err();
    assert!(error.to_string().contains("must be a number"), "{error:#}");

    let error = resolve_password(None, prompter).unwrap_err();
    assert!(error.to_string().contains("must not be empty"), "{error:#}");
}
