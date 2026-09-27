//! Interactive prompts and the resolvers behind "dm db add".

use crate::types::DEFAULT_PORT;
use anyhow::{Context, Result, ensure};
use std::io::IsTerminal;

/// Source of interactive answers.
///
/// Production prompts the controlling terminal; tests script the answers so the
/// interactive branches stay covered without a real TTY.
pub trait Prompter {
    /// Read one visible line, trimming surrounding whitespace.
    fn line(&self, prompt: &str) -> Result<String>;
    /// Read one hidden line, used for passwords.
    fn secret(&self, prompt: &str) -> Result<String>;
}

/// Ask the user on the terminal: the prompt goes to stdout so it appears before
/// the answer is read from stdin.
pub struct TerminalPrompter;

impl Prompter for TerminalPrompter {
    fn line(&self, prompt: &str) -> Result<String> {
        use std::io::Write;
        print!("{prompt}");
        std::io::stdout().flush().context("Flush prompt")?;
        let mut input = String::new();
        std::io::stdin()
            .read_line(&mut input)
            .context("Read input")?;
        Ok(input.trim().to_owned())
    }

    fn secret(&self, prompt: &str) -> Result<String> {
        rpassword::prompt_password(prompt).context("Read hidden input")
    }
}

/// Use the terminal prompter only when stdin is attached to a terminal.
pub(crate) fn terminal_prompter() -> Option<&'static dyn Prompter> {
    static TERMINAL: TerminalPrompter = TerminalPrompter;
    std::io::stdin().is_terminal().then_some(&TERMINAL)
}

/// Resolve a required plain-text field, prompting when it was omitted and a
/// prompter is available (None means stdin is not a terminal).
pub fn resolve_required(
    value: Option<String>,
    prompt: &str,
    missing: &str,
    prompter: Option<&dyn Prompter>,
) -> Result<String> {
    match (value, prompter) {
        (Some(value), _) => Ok(value),
        (None, Some(prompter)) => prompter.line(prompt),
        (None, None) => anyhow::bail!("{missing}"),
    }
}

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
    match (port, configured) {
        (Some(port), _) => Ok(port),
        (None, Some(port)) => Ok(port),
        (None, None) => match prompter {
            Some(prompter) => {
                let value = prompter.line(&format!("Port [{DEFAULT_PORT}]: "))?;
                if value.is_empty() {
                    Ok(DEFAULT_PORT)
                } else {
                    value
                        .parse::<u16>()
                        .with_context(|| format!("Database port must be a number, got '{value}'"))
                }
            }
            None => Ok(DEFAULT_PORT),
        },
    }
}

/// Resolve the password for "dm db add", prompting on the terminal when one was
/// not supplied and the process is attached to a terminal.
pub fn resolve_password(
    password: Option<String>,
    prompter: Option<&dyn Prompter>,
) -> Result<String> {
    match password {
        Some(password) => {
            ensure!(!password.is_empty(), "Database password must not be empty");
            Ok(password)
        }
        None => match prompter {
            Some(prompter) => {
                let password = prompter
                    .secret("Password: ")
                    .context("Read the database password")?;
                ensure!(!password.is_empty(), "Database password must not be empty");
                Ok(password)
            }
            None => anyhow::bail!(
                "Database password is required; pass --password or run from a terminal"
            ),
        },
    }
}

pub(crate) fn prompt_secret(prompter: Option<&dyn Prompter>, prompt: &str) -> Result<String> {
    prompter
        .context("A terminal is required for encrypted import/export passphrases")?
        .secret(prompt)
}

pub(crate) fn prompt_export_passphrase(prompter: Option<&dyn Prompter>) -> Result<String> {
    let first = prompt_secret(prompter, "Export passphrase: ")?;
    let confirmation = prompt_secret(prompter, "Confirm export passphrase: ")?;
    ensure!(first == confirmation, "Export passphrases do not match");
    Ok(first)
}
