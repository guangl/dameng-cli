use anyhow::{Context, Result, ensure};
use log::{debug, info};
use semver::Version;
use serde::Deserialize;
use std::{env, process::Command};

use super::{
    DEFAULT_REPOSITORY,
    archive::{extract_binary, replace_current_executable},
    options::{SUPPORTED_TARGETS, SelfUpdateOptions, SelfUpdateResult},
    verify::{normalize_tag, validate_repository, verify_checksum_file},
};

#[derive(Deserialize)]
struct Release {
    tag_name: String,
}

pub fn self_update(requested: Option<&str>, check_only: bool) -> Result<SelfUpdateResult> {
    self_update_with_options(SelfUpdateOptions {
        version: requested,
        check_only,
        ..SelfUpdateOptions::default()
    })
}

pub fn self_update_with_options(options: SelfUpdateOptions<'_>) -> Result<SelfUpdateResult> {
    let SelfUpdateOptions {
        version: requested,
        check_only,
        force,
        target: target_override,
        repository,
    } = options;
    info!(
        "self-update requested={:?} check_only={check_only} force={force} target={:?}",
        requested, target_override
    );
    let repository = repository
        .map(str::to_owned)
        .or_else(|| env::var("DM_UPDATE_REPOSITORY").ok())
        .unwrap_or_else(|| DEFAULT_REPOSITORY.into());
    validate_repository(&repository)?;
    let temp = tempfile::tempdir()?;
    let tag = if let Some(version) = requested {
        normalize_tag(version)?
    } else {
        let url = format!("https://api.github.com/repos/{repository}/releases/latest");
        let metadata = temp.path().join("release.json");
        download(&url, &metadata)?;
        serde_json::from_slice::<Release>(&crate::support::bounded::file(&metadata, 1024 * 1024)?)?
            .tag_name
    };
    let current = Version::parse(env!("CARGO_PKG_VERSION"))?;
    let available = Version::parse(tag.trim_start_matches('v'))
        .with_context(|| format!("Release tag '{tag}' is not semantic versioning"))?;
    let result = SelfUpdateResult {
        current_version: current.to_string(),
        available_version: available.to_string(),
        updated: false,
    };
    debug!(
        "self-update current={} available={}",
        result.current_version, result.available_version
    );
    if check_only || (available <= current && !force) {
        return Ok(result);
    }

    let target = target_override
        .map(str::to_owned)
        .unwrap_or_else(|| env!("DM_HOST_TARGET").to_owned());
    ensure!(
        SUPPORTED_TARGETS.contains(&target.as_str()),
        "Self-update is not published for target {target}; supported targets are {}",
        SUPPORTED_TARGETS.join(", ")
    );
    let suffix = if cfg!(windows) { ".zip" } else { ".tar.gz" };
    let archive_name = format!("dm-{tag}-{target}{suffix}");
    let base = format!("https://github.com/{repository}/releases/download/{tag}");
    let archive = temp.path().join(&archive_name);
    let checksum = temp.path().join(format!("{archive_name}.sha256"));
    download(&format!("{base}/{archive_name}"), &archive)?;
    download(&format!("{base}/{archive_name}.sha256"), &checksum)?;
    verify_checksum_file(&archive, &checksum)?;
    let replacement = extract_binary(&archive, temp.path(), &tag, &target)?;
    info!("installing dm {available}");
    replace_current_executable(&replacement)?;
    Ok(SelfUpdateResult {
        updated: true,
        ..result
    })
}

fn download(url: &str, destination: &std::path::Path) -> Result<()> {
    info!("downloading {url}");
    let output = crate::support::process::capture(
        Command::new("curl")
            .args([
                "-q",
                "-fsSL",
                "--max-time",
                "120",
                "--retry-max-time",
                "180",
                "--retry",
                "3",
                "--connect-timeout",
                "15",
                "--output",
            ])
            .arg(destination)
            .arg(url),
        std::time::Duration::from_secs(180),
    )
    .context("Self-update requires curl")?;
    ensure!(
        output.status.success(),
        "Could not download {url}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}
