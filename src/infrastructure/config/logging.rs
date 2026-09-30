use super::{Config, configured_value};
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::{
    env,
    path::{Path, PathBuf},
};

/// File-only host diagnostics. Dates and retention are managed by the logger.
#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogSettings {
    pub level: Option<String>,
    /// Absolute directory, or a directory relative to DM_PLUGIN_HOME.
    pub directory: Option<PathBuf>,
    /// Limit of each daily file in MiB; oldest content is removed when full.
    pub max_size_mb: Option<u64>,
}

impl LogSettings {
    pub(super) fn validate(&self) -> Result<()> {
        if let Some(path) = &self.directory {
            ensure!(
                !path.as_os_str().is_empty(),
                "log.directory must not be empty"
            );
        }
        if let Some(size) = self.max_size_mb {
            bytes(size)?;
        }
        Ok(())
    }
}

fn bytes(size: u64) -> Result<u64> {
    ensure!(size > 0, "log.max_size_mb must be greater than zero");
    size.checked_mul(1024 * 1024)
        .context("log.max_size_mb is too large")
}

impl Config {
    /// DM_LOG_DIR > log.directory > `<home>/logs`.
    pub fn log_directory(&self, home: &Path) -> PathBuf {
        let path = env::var_os("DM_LOG_DIR")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| self.log.directory.clone())
            .unwrap_or_else(|| PathBuf::from("logs"));
        home.join(path)
    }

    /// DM_LOG_MAX_SIZE_MB > log.max_size_mb > 5 MiB.
    pub fn log_max_bytes(&self) -> Result<u64> {
        let size = match configured_value(env::var("DM_LOG_MAX_SIZE_MB").ok()) {
            Some(value) => value
                .parse()
                .context("Invalid DM_LOG_MAX_SIZE_MB; use a positive integer")?,
            None => self.log.max_size_mb.unwrap_or(5),
        };
        bytes(size)
    }
}
