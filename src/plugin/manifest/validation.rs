use anyhow::{Result, ensure};

/// Names that are always refused because the host answers them itself.
const RESERVED_NAMES: &[&str] = &[
    "doctor",
    "complete",
    "config",
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

/// Names the host already uses for directories inside its home directory.
///
/// They are refused when a plugin is installed, but not when an installed
/// manifest is read: a release before the per-plugin layout could legitimately
/// contain such a plugin, and refusing to parse it would break `list`,
/// `info`, `update` and `uninstall` for that installation.
pub(crate) const RESERVED_HOME_NAMES: &[&str] = &["cache", "data", "logs", "plugins", "backups"];

/// Reject a plugin name that would collide with a host directory.
pub(crate) fn validate_new_name(name: &str) -> Result<()> {
    ensure!(
        !RESERVED_HOME_NAMES.contains(&name),
        "Plugin name '{name}' is reserved: the host keeps its own '{name}' directory inside DM_PLUGIN_HOME; rename the plugin"
    );
    Ok(())
}

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
