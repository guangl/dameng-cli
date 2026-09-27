//! The "dm ssh add" command.

use crate::auth::resolve_auth;
use crate::config::load_config;
use crate::prompts::{Prompter, resolve_port, resolve_required};
use crate::servers::{Server, upsert_server, validate_name};
use anyhow::{Result, ensure};
use dm_plugin_sdk::Context as PluginContext;
use std::path::PathBuf;

/// The values of "dm ssh add" before defaults and prompts are applied.
pub(crate) struct AddRequest {
    pub(crate) name: Option<String>,
    pub(crate) host: Option<String>,
    pub(crate) port: Option<u16>,
    pub(crate) username: Option<String>,
    pub(crate) password: Option<String>,
    pub(crate) key: Option<PathBuf>,
    pub(crate) passphrase: Option<String>,
}

/// Collect the values of "dm ssh add" from flags, the plugin configuration and prompts.
pub(crate) fn add_server(
    context: &PluginContext,
    request: &AddRequest,
    prompter: Option<&dyn Prompter>,
) -> Result<String> {
    // Values omitted on the command line fall back to the plugin's own
    // configuration file before any prompt.
    let config = load_config(context)?;
    let name = resolve_required(
        request.name.clone(),
        "Name: ",
        "SSH server name is required",
        prompter,
    )?;
    validate_name(&name)?;
    let host = resolve_required(
        request.host.clone(),
        "Host: ",
        "SSH host is required",
        prompter,
    )?;
    ensure!(!host.is_empty(), "SSH host must not be empty");
    let port = resolve_port(request.port.or(config.defaults.port), prompter)?;
    let username = resolve_required(
        request
            .username
            .clone()
            .or(config.defaults.username.clone()),
        "Username: ",
        "SSH username is required",
        prompter,
    )?;
    ensure!(!username.is_empty(), "SSH username must not be empty");
    let key = request
        .key
        .clone()
        .or_else(|| config.defaults.key.clone().map(PathBuf::from));
    let default_method = match config.defaults.auth.as_deref() {
        Some("key") => Some("key"),
        Some(_) => Some("password"),
        None => None,
    };
    let (auth_type, key_path, secret) = resolve_auth(
        context,
        request.password.clone(),
        key,
        request.passphrase.clone(),
        prompter,
        default_method,
    )?;
    upsert_server(
        context,
        &Server {
            name: name.clone(),
            host,
            port,
            username,
            auth_type,
            key_path,
            secret,
        },
    )?;
    Ok(name)
}
