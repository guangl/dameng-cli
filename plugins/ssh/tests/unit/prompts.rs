use dm_plugin_ssh::{resolve_passphrase, resolve_password, resolve_port, resolve_required};

use crate::common::*;

#[test]
fn resolve_password_uses_explicit_value_without_prompting() {
    assert_eq!(
        resolve_password(Some("p@ssw0rd".to_owned()), None).unwrap(),
        "p@ssw0rd"
    );
    assert_eq!(
        resolve_password(Some("p@ssw0rd".to_owned()), None).unwrap(),
        "p@ssw0rd"
    );
}

#[test]
fn resolve_password_rejects_empty_explicit_value() {
    assert!(resolve_password(Some(String::new()), None).is_err());
}

#[test]
fn resolve_password_requires_value_when_not_interactive() {
    let error = resolve_password(None, None).unwrap_err();
    assert!(
        error.to_string().contains("password or key path"),
        "{error}"
    );
}

#[test]
fn resolve_passphrase_handles_explicit_and_missing_values() {
    assert_eq!(
        resolve_passphrase(Some("secret".to_owned()), None)
            .unwrap()
            .as_deref(),
        Some("secret")
    );
    assert_eq!(resolve_passphrase(None, None).unwrap(), None);
    assert_eq!(resolve_passphrase(Some(String::new()), None).unwrap(), None);
}

#[test]
fn resolve_required_uses_value_or_reports_missing() {
    assert_eq!(
        resolve_required(Some("prod".to_owned()), "名称： ", "missing", None).unwrap(),
        "prod"
    );
    let error = resolve_required(None, "名称： ", "name is required", None).unwrap_err();
    assert!(error.to_string().contains("name is required"), "{error}");
}

#[test]
fn resolve_port_defaults_to_22_and_accepts_explicit_value() {
    assert_eq!(resolve_port(None, None).unwrap(), 22);
    assert_eq!(resolve_port(Some(2222), None).unwrap(), 2222);
}

#[test]
fn interactive_prompts_fill_in_missing_fields() {
    let script = Script::new(&["prod", "", "2222"], &[]);
    let prompter = Some(&script as &dyn dm_plugin_ssh::Prompter);
    assert_eq!(
        resolve_required(None, "名称： ", "name is required", prompter).unwrap(),
        "prod"
    );
    // An empty answer to the port prompt keeps the documented default.
    assert_eq!(resolve_port(None, prompter).unwrap(), 22);
    assert_eq!(resolve_port(None, prompter).unwrap(), 2222);
    assert_eq!(resolve_port(Some(2200), prompter).unwrap(), 2200);
}

#[test]
fn interactive_port_rejects_non_numbers() {
    let script = Script::new(&["not-a-port", "2222"], &[]);
    assert_eq!(resolve_port(None, Some(&script)).unwrap(), 2222);
}

#[test]
fn interactive_password_and_passphrase_read_secrets() {
    let script = Script::new(&[], &["p@ssw0rd", "", "retry", "secret", ""]);
    let prompter = Some(&script as &dyn dm_plugin_ssh::Prompter);
    assert_eq!(resolve_password(None, prompter).unwrap(), "p@ssw0rd");
    assert_eq!(resolve_password(None, prompter).unwrap(), "retry");
    assert_eq!(
        resolve_passphrase(None, prompter).unwrap().as_deref(),
        Some("secret")
    );
    assert_eq!(resolve_passphrase(None, prompter).unwrap(), None);
}
