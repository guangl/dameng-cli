use anyhow::{Context, Result, bail, ensure};
use log::debug;
use std::{fs, path::Path, process::Command};

use crate::Manifest;
use crate::support::process::capture;
use std::time::Duration;

use super::{
    github_repository, prebuilt_target_label, progress_bar_for, release_tag_candidates, sha256_file,
};

fn download_prebuilt_asset(url: &str, destination: &Path) -> bool {
    capture(
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
        Duration::from_secs(180),
    )
    .is_ok_and(|output| output.status.success())
}

fn download_optional_prebuilt_checksum(url: &str, destination: &Path) -> Result<bool> {
    let output = capture(
        Command::new("curl")
            .args([
                // Keep curl's behavior deterministic even when the user's .curlrc
                // enables --fail, which would turn an expected 404 into an error.
                "-q",
                "-sSL",
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
            .args(["-w", "%{http_code}"])
            .arg(url),
        Duration::from_secs(180),
    )
    .context("Download prebuilt plugin SHA-256 sidecar")?;
    ensure!(
        output.status.success(),
        "Could not download prebuilt plugin SHA-256 sidecar"
    );
    let status = String::from_utf8_lossy(&output.stdout);
    match status.trim() {
        "200" => Ok(true),
        "404" => Ok(false),
        other => bail!("Unexpected HTTP {other} downloading prebuilt plugin SHA-256 sidecar"),
    }
}

fn verify_optional_prebuilt_checksum(binary_url: &str, binary: &Path) -> Result<()> {
    let checksum_path = binary.with_extension("sha256");
    let downloaded =
        download_optional_prebuilt_checksum(&format!("{binary_url}.sha256"), &checksum_path)?;
    if !downloaded {
        // Security-relevant, so it goes to the terminal and not only to the log
        // file the diagnostics are written to.
        eprintln!("dm: prebuilt plugin has no SHA-256 sidecar; trusting HTTPS transport");
        return Ok(());
    }
    let expected = crate::support::bounded::text(&checksum_path, 4096)?;
    let _ = fs::remove_file(&checksum_path);
    let expected = expected
        .split_whitespace()
        .next()
        .context("Empty prebuilt SHA-256 sidecar")?;
    ensure!(
        expected.len() == 64 && expected.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "Invalid prebuilt SHA-256 sidecar"
    );
    let actual = sha256_file(binary)?;
    ensure!(
        actual.eq_ignore_ascii_case(expected),
        "Prebuilt plugin SHA-256 mismatch"
    );
    Ok(())
}

pub(crate) fn try_download_prebuilt(
    source: Option<&str>,
    manifest: &Manifest,
    revision: Option<&str>,
    destination: &Path,
    progress: bool,
) -> Result<()> {
    let source = source.context("Plugin source is not a GitHub repository")?;
    let (owner, repository) =
        github_repository(source).context("Prebuilt plugins require a GitHub HTTPS source")?;
    let target =
        prebuilt_target_label().context("No prebuilt plugin is published for this host target")?;
    let asset = format!(
        "{}-{}{}",
        manifest.binary_name(),
        target,
        std::env::consts::EXE_SUFFIX
    );
    let bar = progress_bar_for(1, progress);
    bar.set_message("Downloading prebuilt plugin");
    for tag in release_tag_candidates(manifest, revision) {
        let mut assets = vec![asset.clone()];
        // v3.0.1 predates the SDK entrypoint in this repository. Its standalone
        // CLI accepts the same arguments and streams, so it can run under dm.
        // Never infer protocol compatibility for unrelated repositories/releases.
        if owner.eq_ignore_ascii_case("guangl")
            && repository.eq_ignore_ascii_case("dm-database-sqllog2db")
            && manifest.name == "sqllog2db"
            && manifest.version == "3.0.1"
            && tag == "v3.0.1"
        {
            assets.push(format!(
                "sqllog2db-{target}{}",
                std::env::consts::EXE_SUFFIX
            ));
        }
        for candidate in assets {
            let url = format!(
                "https://github.com/{owner}/{repository}/releases/download/{tag}/{candidate}"
            );
            debug!("trying prebuilt asset {url}");
            if download_prebuilt_asset(&url, destination) {
                verify_optional_prebuilt_checksum(&url, destination)?;
                if candidate != asset {
                    eprintln!(
                        "dm: installing sqllog2db v3.0.1 standalone release compatibility binary"
                    );
                }
                bar.inc(1);
                bar.finish_and_clear();
                return Ok(());
            }
        }
    }
    bar.finish_and_clear();
    bail!(
        "No prebuilt plugin '{}' found for {target}; source builds are disabled",
        asset
    )
}
