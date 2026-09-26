//! Host-wide configuration file.
//!
//! The file lives next to the plugin store as `<DM_PLUGIN_HOME>/config.toml`.
//! Every key has an environment-variable equivalent and documented precedence:
//! command-line arguments, then the environment, then this file, then the
//! built-in default. The file cannot relocate the data directory it lives in;
//! use `DM_PLUGIN_HOME` for that.

use crate::infrastructure::store::home_from_env;
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::{
    env, fs,
    path::{Path, PathBuf},
};

/// Configuration file name inside the host data directory.
pub const CONFIG_FILE: &str = "config.toml";

/// Log filter used when neither the environment nor the file sets one.
pub const DEFAULT_LOG_FILTER: &str = "info";

/// Settings read from `<DM_PLUGIN_HOME>/config.toml`.
///
/// Unknown keys are rejected so typos fail loudly instead of being ignored.
#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Log filter with the same syntax as `DM_LOG`, for example `debug` or `dm=debug`.
    #[serde(default)]
    pub log: Option<String>,
    /// GitHub `owner/repository` used by `dm self-update`, same as `DM_UPDATE_REPOSITORY`.
    #[serde(default)]
    pub update_repository: Option<String>,
    /// Release target triple used by `dm self-update`, same as `DM_UPDATE_TARGET`.
    #[serde(default)]
    pub update_target: Option<String>,
    /// Force progress bars on (`true`) or off (`false`) instead of following stderr.
    #[serde(default)]
    pub progress: Option<bool>,
    /// Extra environment variables inherited by plugin processes and hooks.
    #[serde(default)]
    pub plugin_environment: Vec<String>,
}

impl Config {
    /// Path of the configuration file inside a host data directory.
    pub fn path_in(home: &Path) -> PathBuf {
        home.join(CONFIG_FILE)
    }

    /// Parse configuration text, normalizing whitespace and rejecting empty values.
    pub fn from_toml(text: &str) -> Result<Self> {
        let mut config: Self = toml::from_str(text)?;
        for (name, value) in [
            ("log", &mut config.log),
            ("update_repository", &mut config.update_repository),
            ("update_target", &mut config.update_target),
        ] {
            if let Some(raw) = value.take() {
                let trimmed = raw.trim().to_owned();
                ensure!(
                    !trimmed.is_empty(),
                    "Configuration key '{name}' must not be empty; remove it to use the default"
                );
                *value = Some(trimmed);
            }
        }
        config.plugin_environment = valid_environment_names(&config.plugin_environment)?;
        Ok(config)
    }

    /// Load `<home>/config.toml`. A missing file is not an error.
    pub fn load(home: &Path) -> Result<Self> {
        let path = Self::path_in(home);
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => {
                return Err(error).with_context(|| format!("Read {}", path.display()));
            }
        };
        Self::from_toml(&text).with_context(|| format!("Invalid configuration {}", path.display()))
    }

    /// Load the configuration file of the host data directory.
    pub fn from_env() -> Result<Self> {
        Self::load(&home_from_env()?)
    }

    /// Effective log filter: `DM_LOG`, then `RUST_LOG`, then the file, then `info`.
    pub fn log_filter(&self) -> String {
        env::var("DM_LOG")
            .ok()
            .or_else(|| env::var("RUST_LOG").ok())
            .or_else(|| self.log.clone())
            .unwrap_or_else(|| DEFAULT_LOG_FILTER.to_owned())
    }

    /// Effective self-update repository: `DM_UPDATE_REPOSITORY`, then the file.
    ///
    /// `None` means "not configured", which lets the caller apply its own default.
    pub fn update_repository(&self) -> Option<String> {
        configured_value(env::var("DM_UPDATE_REPOSITORY").ok())
            .or_else(|| self.update_repository.clone())
    }

    /// Effective self-update target triple: `DM_UPDATE_TARGET`, then the file.
    pub fn update_target(&self) -> Option<String> {
        configured_value(env::var("DM_UPDATE_TARGET").ok()).or_else(|| self.update_target.clone())
    }

    /// Effective progress-bar preference: `DM_PROGRESS`, then the file.
    ///
    /// `None` means "follow the terminal", which is the built-in default.
    pub fn progress(&self) -> Result<Option<bool>> {
        match configured_value(env::var("DM_PROGRESS").ok()) {
            Some(value) => Ok(Some(parse_switch("DM_PROGRESS", &value)?)),
            None => Ok(self.progress),
        }
    }

    /// Effective extra plugin environment: `DM_PLUGIN_ENVIRONMENT`, then the file.
    ///
    /// The environment variable replaces the file list instead of extending it,
    /// which matches how every other key resolves.
    pub fn plugin_environment(&self) -> Result<Vec<String>> {
        match configured_value(env::var("DM_PLUGIN_ENVIRONMENT").ok()) {
            Some(value) => {
                let names: Vec<String> = value.split(',').map(str::to_owned).collect();
                valid_environment_names(&names)
                    .context("Invalid DM_PLUGIN_ENVIRONMENT; use a comma-separated list of names")
            }
            None => Ok(self.plugin_environment.clone()),
        }
    }
}

/// Treat an unset or blank environment variable as "not configured".
fn configured_value(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// Parse a boolean switch from an environment variable.
fn parse_switch(name: &str, value: &str) -> Result<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        other => anyhow::bail!("{name} must be true or false, got '{other}'"),
    }
}

/// Validate environment variable names and drop duplicates while keeping order.
fn valid_environment_names(names: &[String]) -> Result<Vec<String>> {
    let mut valid: Vec<String> = Vec::new();
    for name in names {
        let name = name.trim();
        ensure!(
            !name.is_empty(),
            "Configuration key 'plugin_environment' must not contain empty names"
        );
        ensure!(
            name.bytes().enumerate().all(|(index, byte)| byte == b'_'
                || byte.is_ascii_alphabetic()
                || (index > 0 && byte.is_ascii_digit())),
            "Configuration key 'plugin_environment' entry '{name}' is not a valid environment variable name"
        );
        if !valid.iter().any(|existing| existing == name) {
            valid.push(name.to_owned());
        }
    }
    Ok(valid)
}
