//! The "dm db add" command.

use crate::domain::types::{DEFAULT_DRIVER, DEFAULT_USERNAME, DatabaseConnection};
use crate::storage::config::load_config;
use crate::storage::connections::{save_connection, validate_name};
use crate::storage::crypto::encrypt;
use crate::ui::prompts::{
    Prompter, resolve_optional, resolve_password, resolve_port, resolve_required,
    resolve_with_default,
};
use anyhow::{Result, ensure};
use dm_plugin_sdk::Context as PluginContext;

/// The values of "dm db add" before defaults and prompts are applied.
pub(crate) struct AddRequest {
    pub(crate) name: Option<String>,
    pub(crate) host: Option<String>,
    pub(crate) port: Option<u16>,
    pub(crate) username: Option<String>,
    pub(crate) password: Option<String>,
    pub(crate) schema: Option<String>,
    pub(crate) driver: Option<String>,
}

/// Collect the values of "dm db add" from flags, the plugin configuration, prompts.
pub(crate) fn add_connection(
    context: &PluginContext,
    request: &AddRequest,
    prompter: Option<&dyn Prompter>,
    replace: bool,
    yes: bool,
) -> Result<String> {
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
    let existing = crate::storage::connections::load_connections(context)?
        .into_iter()
        .any(|entry| entry.name == name);
    ensure!(
        !existing || replace,
        "同名连接 '{name}' 已存在，请使用 edit 修改，或 add --replace 覆盖"
    );
    let host = resolve_required(
        request.host.clone(),
        "地址: ",
        "Database host is required",
        prompter,
    )?;
    ensure!(!host.trim().is_empty(), "Database host must not be empty");
    let port = resolve_port(request.port.or(config.defaults.port), None, prompter)?;
    let username = resolve_with_default(
        request.username.clone(),
        config.defaults.username.clone(),
        DEFAULT_USERNAME,
        "用户名: ",
        prompter,
    )?;
    let driver = resolve_with_default(
        request.driver.clone(),
        config.defaults.driver.clone(),
        DEFAULT_DRIVER,
        "驱动: ",
        prompter,
    )?;
    let schema = resolve_optional(
        request.schema.clone().or(config.defaults.schema.clone()),
        "Schema（留空使用登录默认值）: ",
        prompter,
    )?;
    let password = resolve_password(request.password.clone(), prompter)?;
    if prompter.is_some() {
        dm_plugin_support::interaction::confirm(
            prompter,
            yes,
            &format!("保存连接 {name}：{username}@{host}:{port}（密码已隐藏）？"),
        )?;
    }
    save_connection(
        context,
        &DatabaseConnection {
            name: name.clone(),
            host: host.trim().to_owned(),
            port,
            username: username.trim().to_owned(),
            schema,
            driver: driver.trim().to_owned(),
            secret: Some(encrypt(context, password.as_bytes())?),
        },
        replace,
    )?;
    Ok(name)
}
