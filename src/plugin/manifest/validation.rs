use anyhow::{Result, ensure};

const RESERVED_NAMES: &[&str] = &[
    "doctor",
    "completions",
    "help",
    "info",
    "install",
    "list",
    "self-update",
    "uninstall",
    "update",
    "version",
];

pub fn validate_name(name: &str) -> Result<()> {
    ensure!(
        !name.is_empty() && name.len() <= 64,
        "Plugin name must contain 1–64 characters"
    );
    ensure!(
        name.as_bytes()[0].is_ascii_lowercase()
            && name
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-'),
        "Plugin name must start with a lowercase letter and contain only a-z, 0-9 or '-'"
    );
    ensure!(
        !RESERVED_NAMES.contains(&name),
        "Plugin name '{name}' is reserved"
    );
    ensure!(
        !matches!(name, "con" | "prn" | "aux" | "nul")
            && !(name.len() == 4
                && (name.starts_with("com") || name.starts_with("lpt"))
                && matches!(name.as_bytes()[3], b'1'..=b'9')),
        "Plugin name '{name}' is not portable"
    );
    Ok(())
}

pub(super) fn validate_hook(hook: &str) -> Result<()> {
    ensure!(!hook.is_empty(), "Hook path must not be empty");
    ensure!(
        !hook.starts_with('/') && !hook.starts_with('\\') && !hook.contains(':'),
        "Hook path must be relative"
    );
    for component in hook.split(['/', '\\']) {
        ensure!(
            !component.is_empty() && component != "." && component != "..",
            "Hook path must be a relative executable path"
        );
        ensure!(
            component
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')),
            "Hook path contains unsupported characters"
        );
    }
    Ok(())
}
