use anyhow::{Context, Result, bail};
use log::debug;
use std::{path::Path, process::Command};

use crate::Manifest;
use crate::support::process::capture;
use std::time::Duration;

use super::{github_repository, prebuilt_target_label, progress_bar_for, release_tag_candidates};

fn download_prebuilt_asset(url: &str, destination: &Path) -> Result<()> {
    let output = capture(
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
    .context("Download prebuilt plugin requires curl")?;
    anyhow::ensure!(
        output.status.success(),
        "Prebuilt plugin download failed: {}",
        String::from_utf8_lossy(&output.stderr)
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
            if let Some(hash) = crate::support::github_release::asset_digest(
                &format!("{owner}/{repository}"),
                &tag,
                &candidate,
            )? {
                download_prebuilt_asset(&url, destination)?;
                crate::support::github_release::verify_file(destination, &hash)?;
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
