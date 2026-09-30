//! Interactive prompts and the resolvers behind "dm db add".

use crate::domain::types::DEFAULT_PORT;
use anyhow::{Context, Result, ensure};

pub(crate) use dm_plugin_support::interaction::terminal_prompter;
pub use dm_plugin_support::interaction::{Prompter, TerminalPrompter, resolve_required};

/// Resolve a value that falls back to the plugin configuration and then to a
/// documented default; an empty interactive answer keeps that default.
pub fn resolve_with_default(
    value: Option<String>,
    configured: Option<String>,
    fallback: &str,
    prompt: &str,
    prompter: Option<&dyn Prompter>,
) -> Result<String> {
    let value = match (value, configured) {
        (Some(value), _) => value,
        (None, Some(configured)) => configured,
        (None, None) => match prompter {
            Some(prompter) => {
                let answer = prompter.line(&format!("{prompt} [{fallback}]: "))?;
                if answer.is_empty() {
                    fallback.to_owned()
                } else {
                    answer
                }
            }
            None => fallback.to_owned(),
        },
    };
    ensure!(!value.trim().is_empty(), "A value must not be empty");
    Ok(value)
}

/// Resolve an optional value; an empty answer means "not set".
pub fn resolve_optional(
    value: Option<String>,
    prompt: &str,
    prompter: Option<&dyn Prompter>,
) -> Result<Option<String>> {
    let value = match (value, prompter) {
        (Some(value), _) => Some(value),
        (None, Some(prompter)) => Some(prompter.line(prompt)?),
        (None, None) => None,
    };
    Ok(value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty()))
}

/// Resolve the port, defaulting to 5236 when it is not configured.
pub fn resolve_port(
    port: Option<u16>,
    configured: Option<u16>,
    prompter: Option<&dyn Prompter>,
) -> Result<u16> {
    dm_plugin_support::interaction::port(port.or(configured), DEFAULT_PORT, prompter)
}

/// Resolve the password for "dm db add", prompting on the terminal when one was
/// not supplied and the process is attached to a terminal.
pub fn resolve_password(
    password: Option<String>,
    prompter: Option<&dyn Prompter>,
) -> Result<String> {
    dm_plugin_support::interaction::password(
        password,
        prompter,
        "Database password is required; pass --password or run from a terminal",
    )
}

pub(crate) fn prompt_secret(prompter: Option<&dyn Prompter>, prompt: &str) -> Result<String> {
    prompter
        .context("A terminal is required for encrypted import/export passphrases")?
        .secret(prompt)
}

pub(crate) fn prompt_export_passphrase(prompter: Option<&dyn Prompter>) -> Result<String> {
    let first = prompt_secret(prompter, "导出口令: ")?;
    let confirmation = prompt_secret(prompter, "再次输入导出口令: ")?;
    ensure!(first == confirmation, "Export passphrases do not match");
    Ok(first)
}
