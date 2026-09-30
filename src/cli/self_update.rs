//! The `dm self-update` subcommand.

use crate::{Config, SelfUpdateOptions, self_update_with_options};
use anyhow::Result;

/// Replace the running host binary from its GitHub Release.
pub(super) fn run(
    config: &Config,
    version: Option<&str>,
    check: bool,
    force: bool,
    target: Option<&str>,
    json: bool,
) -> Result<()> {
    let repository = config.update_repository();
    let configured_target = config.update_target();
    let result = self_update_with_options(SelfUpdateOptions {
        version,
        check_only: check,
        force,
        target: target.or(configured_target.as_deref()),
        repository: repository.as_deref(),
    })?;
    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else if result.updated {
        println!(
            "已升级 dm：{} → {}",
            result.current_version, result.available_version
        );
    } else if result.current_version == result.available_version {
        println!("dm {} 已是最新版本", result.current_version);
    } else {
        println!(
            "当前 dm {}，可升级至 {}",
            result.current_version, result.available_version
        );
    }
    Ok(())
}
