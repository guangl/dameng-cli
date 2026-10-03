use dameng_cli::support::interaction::{Prompter, confirm};
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
}

#[test]
fn confirmations_require_an_explicit_yes_and_default_to_cancel() {
    assert!(confirm(None, false, "remove").is_err());
    assert!(confirm(Some(&Script::new(&[""])), false, "remove").is_err());
    assert!(confirm(Some(&Script::new(&["y"])), false, "remove").is_ok());
    assert!(confirm(Some(&Script::new(&["YES"])), false, "remove").is_ok());
    assert!(confirm(Some(&Script::new(&["是"])), false, "remove").is_ok());
    assert!(confirm(Some(&Script::new(&["no"])), false, "remove").is_err());
    assert!(confirm(None, true, "remove").is_ok());
    assert!(Script::new(&[]).line("prompt").is_err());
}
