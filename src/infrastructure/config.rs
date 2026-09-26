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
        env::var("DM_UPDATE_REPOSITORY")
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .or_else(|| self.update_repository.clone())
    }
}
