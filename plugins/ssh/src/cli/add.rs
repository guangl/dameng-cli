//! The "dm ssh add" command.

use crate::domain::auth::resolve_auth;
use crate::storage::config::load_config;
use crate::storage::servers::{Server, save_server, validate_name};
use crate::ui::prompts::{Prompter, resolve_port, resolve_required};
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
    replace: bool,
    yes: bool,
) -> Result<String> {
    // Values omitted on the command line fall back to the plugin's own
    // configuration file before any prompt.
    let config = load_config(context)?;
    let name = match request.name.clone() {
        Some(name) => {
            validate_name(&name)?;
            name
        }
        None => dm_plugin_support::interaction::validated(
            prompter.ok_or_else(|| anyhow::anyhow!("连接名称必填"))?,
            "连接名称: ",
            |name| {
                validate_name(name)?;
                Ok(name.to_owned())
            },
        )?,
    };
    let existing = crate::storage::servers::load_servers(context)?
        .into_iter()
        .any(|entry| entry.name == name);
    ensure!(
        !existing || replace,
        "同名连接 '{name}' 已存在，请使用 edit 修改，或 add --replace 覆盖"
    );
    let host = resolve_required(
        request.host.clone(),
        "地址: ",
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
        "用户名: ",
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
    if prompter.is_some() {
        dm_plugin_support::interaction::confirm(
            prompter,
            yes,
            &format!("保存连接 {name}：{username}@{host}:{port}（密码已隐藏）？"),
        )?;
    }
    save_server(
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
        replace,
    )?;
    Ok(name)
}
