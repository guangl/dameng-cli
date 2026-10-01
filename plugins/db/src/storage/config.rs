//! The plugin's own config.toml: [defaults] and [connect].

use anyhow::{Context, Result, ensure};
use dm_plugin_sdk::Context as PluginContext;
use serde::Deserialize;
use std::path::PathBuf;

/// Settings read from the plugin's own config.toml in its config directory.
///
/// The host only creates and passes the directory; the schema below belongs to
/// the plugin, so these keys never appear in the host configuration file.
#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DbConfig {
    /// Values used by "dm db add" when a flag is omitted.
    #[serde(default)]
    pub defaults: DbDefaults,
    /// How "dm db test" and "dm db exec" connect.
    #[serde(default)]
    pub connect: DbConnect,
}

/// The \[defaults\] table.
#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DbDefaults {
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub driver: Option<String>,
    #[serde(default)]
    pub schema: Option<String>,
}

/// The \[connect\] table.
#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DbConnect {
    #[serde(default)]
    pub timeout: Option<u64>,
    #[serde(default)]
    pub probe: Option<String>,
}

/// Path of the plugin's own configuration file.
pub fn config_path(context: &PluginContext) -> PathBuf {
    context.config_file()
}

/// Load the plugin configuration. A missing file means "all defaults".
pub fn load_config(context: &PluginContext) -> Result<DbConfig> {
    let path = config_path(context);
    let text =
        match dm_plugin_support::bounded::text(&path, dm_plugin_support::bounded::CONFIG_LIMIT) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(DbConfig::default());
            }
            Err(error) => return Err(error).with_context(|| format!("Read {}", path.display())),
        };
    let config: DbConfig = toml::from_str(&text).with_context(|| {
        format!(
            "Invalid database plugin configuration {}; supported tables are [defaults] and [connect]",
            path.display()
        )
    })?;
    if let Some(port) = config.defaults.port {
        ensure!(
            port > 0,
            "Database plugin configuration {}: defaults.port must not be zero",
            path.display()
        );
    }
    if let Some(timeout) = config.connect.timeout {
        ensure!(
            timeout > 0,
            "Database plugin configuration {}: connect.timeout must be greater than zero",
            path.display()
        );
    }
    for (key, value) in [
        ("defaults.username", config.defaults.username.as_deref()),
        ("defaults.driver", config.defaults.driver.as_deref()),
        ("defaults.schema", config.defaults.schema.as_deref()),
        ("connect.probe", config.connect.probe.as_deref()),
    ] {
        ensure!(
            !value.is_some_and(|value| value.trim().is_empty()),
            "Database plugin configuration {}: {key} must not be empty",
            path.display()
        );
    }
    Ok(config)
}
