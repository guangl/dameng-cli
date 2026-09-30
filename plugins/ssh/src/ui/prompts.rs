use anyhow::{Context, Result, ensure};

pub(crate) use dm_plugin_support::interaction::terminal_prompter;
pub use dm_plugin_support::interaction::{Prompter, TerminalPrompter, resolve_required};

/// Resolve the SSH port, defaulting to 22 when omitted.
pub fn resolve_port(port: Option<u16>, prompter: Option<&dyn Prompter>) -> Result<u16> {
    dm_plugin_support::interaction::port(port, 22, prompter)
}

/// Resolve the password for `add`, prompting on the terminal when one was not
/// supplied and the process is attached to a terminal.
pub fn resolve_password(
    password: Option<String>,
    prompter: Option<&dyn Prompter>,
) -> Result<String> {
    dm_plugin_support::interaction::password(
        password,
        prompter,
        "SSH password or key path is required; pass --password or --key, or run from a terminal",
    )
}

/// Read one hidden answer for the encrypted export and import passphrases.
pub(crate) fn prompt_secret(prompter: Option<&dyn Prompter>, prompt: &str) -> Result<String> {
    prompter
        .context("A terminal is required for encrypted import/export passphrases")?
        .secret(prompt)
}

/// Read the export passphrase twice so a typo cannot make the file unreadable.
pub(crate) fn prompt_export_passphrase(prompter: Option<&dyn Prompter>) -> Result<String> {
    let first = prompt_secret(prompter, "导出口令: ")?;
    let confirmation = prompt_secret(prompter, "再次输入导出口令: ")?;
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
                    .secret("私钥口令（留空表示无口令）: ")
                    .context("Read SSH key passphrase")?;
                Ok((!passphrase.is_empty()).then_some(passphrase))
            }
            None => Ok(None),
        },
    }
}
