use anyhow::{Context, Result, ensure};
use dm_plugin_sdk::Context as PluginContext;
use serde::Deserialize;
use std::path::PathBuf;

/// Default connection timeout in seconds for `dm ssh test`.
pub(crate) const DEFAULT_CONNECT_TIMEOUT: u64 = 10;

/// Settings read from the plugin's own `<config_dir>/config.toml`.
///
/// The host only creates and passes the directory; the schema below is the
/// plugin's own, so these keys never appear in the host configuration file.
#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SshConfig {
    /// Values used by `dm ssh add` when a flag is omitted.
    #[serde(default)]
    pub defaults: SshDefaults,
    /// How `dm ssh test` connects.
    #[serde(default)]
    pub test: SshTestSettings,
}

/// `[defaults]` table.
#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SshDefaults {
    /// Port used when `--port` is omitted and the prompt is answered with Enter.
    #[serde(default)]
    pub port: Option<u16>,
    /// Username used when `--username` is omitted and the prompt is answered with Enter.
    #[serde(default)]
    pub username: Option<String>,
    /// Authentication method preselected by the interactive prompt: `password` or `key`.
    #[serde(default)]
    pub auth: Option<String>,
    /// Private key path used when `--key` is omitted.
    #[serde(default)]
    pub key: Option<String>,
}

/// `[test]` table.
#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SshTestSettings {
    /// Connection timeout in seconds for `dm ssh test`; defaults to 10.
    #[serde(default)]
    pub connect_timeout: Option<u64>,
}

/// Path of the plugin's own configuration file.
pub fn config_path(context: &PluginContext) -> PathBuf {
    context.config_dir.join(dm_plugin_sdk::CONFIG_FILE)
}

/// Load the plugin configuration. A missing file means "all defaults".
pub fn load_config(context: &PluginContext) -> Result<SshConfig> {
    let path = config_path(context);
    let text =
        match dm_plugin_support::bounded::text(&path, dm_plugin_support::bounded::CONFIG_LIMIT) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(SshConfig::default());
            }
            Err(error) => return Err(error).with_context(|| format!("Read {}", path.display())),
        };
    let config: SshConfig = toml::from_str(&text).with_context(|| {
        format!(
            "Invalid SSH plugin configuration {}; supported tables are [defaults] and [test]",
            path.display()
        )
    })?;
    if let Some(auth) = config.defaults.auth.as_deref() {
        ensure!(
            matches!(auth, "password" | "key"),
            "SSH plugin configuration {}: defaults.auth must be 'password' or 'key', got '{auth}'",
            path.display()
        );
    }
    if let Some(timeout) = config.test.connect_timeout {
        ensure!(
            timeout > 0,
            "SSH plugin configuration {}: test.connect_timeout must be greater than zero",
            path.display()
        );
    }
    ensure!(
        !config
            .defaults
            .username
            .as_deref()
            .is_some_and(|value| value.trim().is_empty()),
        "SSH plugin configuration {}: defaults.username must not be empty",
        path.display()
    );
    ensure!(
        !config
            .defaults
            .key
            .as_deref()
            .is_some_and(|value| value.trim().is_empty()),
        "SSH plugin configuration {}: defaults.key must not be empty",
        path.display()
    );
    Ok(config)
}
