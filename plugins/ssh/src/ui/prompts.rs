use anyhow::{Context, Result, ensure};
use std::io::IsTerminal;

use crate::domain::auth::AUTH_REQUIRED;

/// Source of interactive answers.
///
/// Production prompts the controlling terminal; tests script the answers so the
/// interactive branches stay covered without a real TTY.
pub trait Prompter {
    /// Read one visible line, trimming surrounding whitespace.
    fn line(&self, prompt: &str) -> Result<String>;
    /// Read one hidden line, used for passwords and key passphrases.
    fn secret(&self, prompt: &str) -> Result<String>;
}

/// Ask the user on the controlling terminal: the prompt is written to stdout so
/// it appears before the answer is read from stdin.
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
/// prompter is available (`None` means stdin is not a terminal).
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

/// Resolve the SSH port, defaulting to 22 when omitted.
pub fn resolve_port(port: Option<u16>, prompter: Option<&dyn Prompter>) -> Result<u16> {
    match port {
        Some(port) => Ok(port),
        None => match prompter {
            Some(prompter) => {
                let value = prompter.line("Port [22]: ")?;
                if value.is_empty() {
                    Ok(22)
                } else {
                    value
                        .parse::<u16>()
                        .with_context(|| format!("SSH port must be a number, got '{value}'"))
                }
            }
            None => Ok(22),
        },
    }
}

/// Resolve the password for `add`, prompting on the terminal when one was not
/// supplied and the process is attached to a terminal.
pub fn resolve_password(
    password: Option<String>,
    prompter: Option<&dyn Prompter>,
) -> Result<String> {
    match password {
        Some(password) => {
            ensure!(!password.is_empty(), "SSH password must not be empty");
            Ok(password)
        }
        None => match prompter {
            Some(prompter) => {
                let password = prompter.secret("Password: ").context("Read SSH password")?;
                ensure!(!password.is_empty(), "SSH password must not be empty");
                Ok(password)
            }
            None => anyhow::bail!(AUTH_REQUIRED),
        },
    }
}

/// Read one hidden answer for the encrypted export and import passphrases.
pub(crate) fn prompt_secret(prompter: Option<&dyn Prompter>, prompt: &str) -> Result<String> {
    prompter
        .context("A terminal is required for encrypted import/export passphrases")?
        .secret(prompt)
}

/// Read the export passphrase twice so a typo cannot make the file unreadable.
pub(crate) fn prompt_export_passphrase(prompter: Option<&dyn Prompter>) -> Result<String> {
    let first = prompt_secret(prompter, "Export passphrase: ")?;
    let confirmation = prompt_secret(prompter, "Confirm export passphrase: ")?;
    ensure!(first == confirmation, "Export passphrases do not match");
    Ok(first)
}

/// Resolve the optional key passphrase for `add`. An empty passphrase means the
/// key is not protected by one. Prompting only happens on a terminal.
pub fn resolve_passphrase(
    passphrase: Option<String>,
    prompter: Option<&dyn Prompter>,
) -> Result<Option<String>> {
    match passphrase {
        Some(passphrase) if passphrase.is_empty() => Ok(None),
        Some(passphrase) => Ok(Some(passphrase)),
        None => match prompter {
            Some(prompter) => {
                let passphrase = prompter
                    .secret("Passphrase (leave empty for none): ")
                    .context("Read SSH key passphrase")?;
                Ok((!passphrase.is_empty()).then_some(passphrase))
            }
            None => Ok(None),
        },
    }
}
