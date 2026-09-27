//! The "dm db add" command.

use crate::config::load_config;
use crate::connections::{upsert_connection, validate_name};
use crate::crypto::encrypt;
use crate::prompts::{
    Prompter, resolve_optional, resolve_password, resolve_port, resolve_required,
    resolve_with_default,
};
use crate::types::{DEFAULT_DRIVER, DEFAULT_USERNAME, DatabaseConnection};
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
) -> Result<String> {
    let config = load_config(context)?;
    let name = resolve_required(
        request.name.clone(),
        "Name: ",
        "Connection name is required",
        prompter,
    )?;
    validate_name(&name)?;
    let host = resolve_required(
        request.host.clone(),
        "Host: ",
        "Database host is required",
        prompter,
    )?;
    ensure!(!host.trim().is_empty(), "Database host must not be empty");
    let port = resolve_port(request.port.or(config.defaults.port), None, prompter)?;
    let username = resolve_with_default(
        request.username.clone(),
        config.defaults.username.clone(),
        DEFAULT_USERNAME,
        "Username: ",
        prompter,
    )?;
    let driver = resolve_with_default(
        request.driver.clone(),
        config.defaults.driver.clone(),
        DEFAULT_DRIVER,
        "Driver: ",
        prompter,
    )?;
    let schema = resolve_optional(
        request.schema.clone().or(config.defaults.schema.clone()),
        "Schema (leave empty for the login default): ",
        prompter,
    )?;
    let password = resolve_password(request.password.clone(), prompter)?;
    upsert_connection(
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
    )?;
    Ok(name)
}
