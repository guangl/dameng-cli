use anyhow::{Context, Result, ensure};
use log::info;
use std::{path::PathBuf, process::Command};

use super::progress_bar_for;

pub(crate) fn checkout_git(
    source: &str,
    revision: Option<&str>,
    progress: bool,
) -> Result<(tempfile::TempDir, PathBuf, String)> {
    ensure!(
        source.starts_with("https://") && source.len() > 8,
        "Source must be an HTTPS Git repository URL"
    );
    if let Some(revision) = revision {
        ensure!(
            !revision.is_empty()
                && !revision.starts_with('-')
                && !revision.chars().any(char::is_whitespace),
            "Git revision must be nonempty and contain no whitespace"
        );
    }
    let checkout = tempfile::tempdir()?;
    let destination = checkout.path().join("source");
    let bar = progress_bar_for(2, progress);
    bar.set_message("Cloning plugin repository");
    info!("cloning {source}");
    let mut clone = Command::new("git");
    clone.args([
        "-c",
        "core.hooksPath=/dev/null",
        "-c",
        "protocol.allow=never",
        "-c",
        "protocol.https.allow=always",
        "clone",
    ]);
    if revision.is_none() {
        clone.args(["--depth", "1"]);
    }
    let output = clone
        .arg("--")
        .arg(source)
        .arg(&destination)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .context("Fetch plugin; HTTPS installation requires Git")?;
    ensure!(
        output.status.success(),
        "Git could not fetch the plugin\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    bar.inc(1);
    if let Some(revision) = revision {
        info!("checking out revision {revision}");
        let output = Command::new("git")
            .args(["-c", "core.hooksPath=/dev/null", "checkout", "--detach"])
            .arg(revision)
            .current_dir(&destination)
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()?;
        ensure!(
            output.status.success(),
            "Git revision '{revision}' could not be checked out\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&destination)
        .output()?;
    ensure!(
        output.status.success(),
        "Could not resolve plugin Git revision"
    );
    bar.inc(1);
    bar.finish_and_clear();
    let resolved = String::from_utf8(output.stdout)?.trim().to_owned();
    Ok((checkout, destination, resolved))
}
