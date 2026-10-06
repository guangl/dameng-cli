use anyhow::{Result, ensure};
use serde::Deserialize;

use super::{Config, parse::configured_value};

/// Default toolchain for source builds without a plugin toolchain file.
#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildSettings {
    pub toolchain: Option<String>,
}

impl BuildSettings {
    pub(super) fn validate(&self) -> Result<()> {
        if let Some(value) = &self.toolchain {
            validate_toolchain(value)
                .map_err(|error| anyhow::anyhow!("Invalid build.toolchain: {error}"))?;
        }
        Ok(())
    }
}

/// Only complete stable release numbers are accepted, never paths or aliases.
pub(crate) fn validate_toolchain(value: &str) -> Result<()> {
    let version = semver::Version::parse(value).map_err(|_| {
        anyhow::anyhow!(
            "Rust toolchain must be a fixed stable version such as 1.99.0, got '{value}'"
        )
    })?;
    ensure!(
        version.major == 1 && version.pre.is_empty() && version.build.is_empty(),
        "Rust toolchain must be a fixed stable version such as 1.99.0, got '{value}'"
    );
    Ok(())
}

impl Config {
    pub(crate) fn default_build_toolchain(&self) -> String {
        configured_value(std::env::var("DM_BUILD_TOOLCHAIN").ok())
            .or_else(|| self.build.toolchain.clone())
            .unwrap_or_else(|| env!("CARGO_PKG_RUST_VERSION").into())
    }

    /// Host fallback: environment, configuration, then the host's MSRV.
    /// A CLI override or plugin toolchain file takes precedence over this value.
    pub fn build_toolchain(&self) -> Result<String> {
        let value = self.default_build_toolchain();
        validate_toolchain(&value)?;
        Ok(value)
    }
}
