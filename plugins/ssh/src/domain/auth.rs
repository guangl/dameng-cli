use anyhow::{Result, ensure};
use dm_plugin_sdk::Context as PluginContext;
use std::path::PathBuf;

use crate::storage::crypto::encrypt;
use crate::ui::prompts::{Prompter, resolve_passphrase, resolve_password};

pub(crate) const AUTH_REQUIRED: &str =
    "SSH password or key path is required; pass --password or --key, or run from a terminal";

/// Resolve the authentication method and its encrypted secret for `add`.
/// Explicit `--key` or `--password` wins; otherwise an interactive terminal
/// chooses the method and enters the matching secret.
pub fn resolve_auth(
    context: &PluginContext,
    password: Option<String>,
    key: Option<PathBuf>,
    passphrase: Option<String>,
    prompter: Option<&dyn Prompter>,
    default_method: Option<&'static str>,
) -> Result<(String, Option<String>, Option<String>)> {
    if let Some(key_path) = key {
        ensure!(
            !key_path.as_os_str().is_empty(),
            "SSH key path must not be empty"
        );
        let passphrase = resolve_passphrase(passphrase, prompter)?;
        return Ok((
            "key".to_owned(),
            Some(key_path.display().to_string()),
            passphrase
                .map(|passphrase| encrypt(context, passphrase.as_bytes()))
                .transpose()?,
        ));
    }
    if let Some(password) = password {
        let password = resolve_password(Some(password), prompter)?;
        return Ok((
            "password".to_owned(),
            None,
            Some(encrypt(context, password.as_bytes())?),
        ));
    }
    let prompter = match prompter {
        Some(prompter) => prompter,
        None => anyhow::bail!(AUTH_REQUIRED),
    };
    // `[defaults] auth` from the plugin configuration preselects the method.
    let method = dm_plugin_support::interaction::validated(
        prompter,
        &format!(
            "认证方式 [password/key] ({}): ",
            default_method.unwrap_or("password")
        ),
        |answer| {
            let method = if answer.is_empty() {
                default_method.unwrap_or("password")
            } else {
                answer
            };
            ensure!(
                matches!(method, "password" | "key"),
                "Unknown authentication method '{method}'; choose 'password' or 'key'"
            );
            Ok(method.to_owned())
        },
    )?;
    match method.as_str() {
        "password" => {
            let password = resolve_password(None, Some(prompter))?;
            Ok((
                "password".to_owned(),
                None,
                Some(encrypt(context, password.as_bytes())?),
            ))
        }
        "key" => {
            let key_path = dm_plugin_support::interaction::resolve_required(
                None,
                "本机私钥路径: ",
                "SSH key path must not be empty",
                Some(prompter),
            )?;
            let passphrase = resolve_passphrase(None, Some(prompter))?;
            Ok((
                "key".to_owned(),
                Some(key_path),
                passphrase
                    .map(|passphrase| encrypt(context, passphrase.as_bytes()))
                    .transpose()?,
            ))
        }
        other => {
            anyhow::bail!("Unknown authentication method '{other}'; choose 'password' or 'key'")
        }
    }
}
