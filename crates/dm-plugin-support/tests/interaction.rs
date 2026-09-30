use dm_plugin_support::interaction::*;
use std::{cell::RefCell, collections::VecDeque};
struct Script(RefCell<VecDeque<String>>);
impl Script {
    fn new(values: &[&str]) -> Self {
        Self(RefCell::new(values.iter().map(|s| s.to_string()).collect()))
    }
}
impl Prompter for Script {
    fn line(&self, _: &str) -> anyhow::Result<String> {
        self.0
            .borrow_mut()
            .pop_front()
            .ok_or_else(|| anyhow::anyhow!("closed"))
    }
    fn secret(&self, prompt: &str) -> anyhow::Result<String> {
        self.line(prompt)
    }
}
#[test]
fn required_port_and_secret_retry_only_interactive_answers() {
    let script = Script::new(&["", "prod", "oops", "0", "", "", "pw"]);
    assert_eq!(
        resolve_required(None, "name", "missing", Some(&script)).unwrap(),
        "prod"
    );
    assert_eq!(port(None, 22, Some(&script)).unwrap(), 22);
    assert_eq!(password(None, Some(&script), "missing").unwrap(), "pw");
    assert!(port(Some(0), 22, None).is_err());
    assert_eq!(port(Some(2200), 22, None).unwrap(), 2200);
    assert_eq!(port(None, 22, None).unwrap(), 22);
    assert!(password(Some(String::new()), None, "missing").is_err());
    assert_eq!(password(Some("pw".into()), None, "missing").unwrap(), "pw");
    assert!(password(None, None, "missing").is_err());
    assert!(resolve_required(Some(" ".into()), "n", "missing", None).is_err());
    assert!(resolve_required(None, "n", "missing", None).is_err());
    assert_eq!(
        resolve_required(Some(" x ".into()), "n", "missing", None).unwrap(),
        "x"
    );
    assert_eq!(port(None, 22, Some(&Script::new(&["2200"]))).unwrap(), 2200);
}
#[test]
fn edits_keep_old_values_and_confirmations_default_to_cancel() {
    assert_eq!(edit_field(None, "old", "host", None).unwrap(), "old");
    assert_eq!(
        edit_field(None, "old", "host", Some(&Script::new(&[""]))).unwrap(),
        "old"
    );
    assert_eq!(
        edit_field(None, "old", "host", Some(&Script::new(&["new"]))).unwrap(),
        "new"
    );
    assert_eq!(
        edit_field(Some("new".into()), "old", "host", None).unwrap(),
        "new"
    );
    assert!(edit_field(Some(" ".into()), "old", "host", None).is_err());
    assert!(confirm(None, false, "remove").is_err());
    assert!(confirm(Some(&Script::new(&[""])), false, "remove").is_err());
    assert!(confirm(Some(&Script::new(&["yes"])), false, "remove").is_ok());
    assert!(confirm(None, true, "remove").is_ok());
}
#[test]
fn selector_accepts_exact_names_and_unique_searches() {
    let names = vec!["prod".into(), "stage".into()];
    assert_eq!(
        select_name(Some("prod".into()), &names, None).unwrap(),
        "prod"
    );
    assert!(
        select_name(Some("prodd".into()), &names, None)
            .unwrap_err()
            .to_string()
            .contains("prod")
    );
    assert!(select_name(None, &[], None).is_err());
    assert_eq!(select_name(None, &["prod".into()], None).unwrap(), "prod");
    assert!(select_name(None, &names, None).is_err());
    assert_eq!(
        select_name(None, &names, Some(&Script::new(&["", "pro"]))).unwrap(),
        "prod"
    );
    assert_eq!(
        select_name(None, &names, Some(&Script::new(&["stage"]))).unwrap(),
        "stage"
    );
    assert_eq!(suggestions("prodd", &names), "prod, stage");
}
