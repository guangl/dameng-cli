use anyhow::{Context, Result, ensure};
use log::{debug, info};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{env, fs, path::PathBuf, process::Command};

const DEFAULT_REPOSITORY: &str = "guangl/dameng-cli";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SelfUpdateResult {
    pub current_version: String,
    pub available_version: String,
    pub updated: bool,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
}

pub fn self_update(requested: Option<&str>, check_only: bool) -> Result<SelfUpdateResult> {
    self_update_with_options(requested, check_only, false, None)
}

pub fn self_update_with_options(
    requested: Option<&str>,
    check_only: bool,
    force: bool,
    target_override: Option<&str>,
) -> Result<SelfUpdateResult> {
    info!(
        "self-update requested={:?} check_only={check_only} force={force} target={:?}",
        requested, target_override
    );
    let repository = env::var("DM_UPDATE_REPOSITORY").unwrap_or_else(|_| DEFAULT_REPOSITORY.into());
    validate_repository(&repository)?;
    let temp = tempfile::tempdir()?;
    let tag = if let Some(version) = requested {
        normalize_tag(version)?
    } else {
        let url = format!("https://api.github.com/repos/{repository}/releases/latest");
        let metadata = temp.path().join("release.json");
        download(&url, &metadata)?;
        serde_json::from_slice::<Release>(&fs::read(metadata)?)?.tag_name
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
        matches!(
            target.as_str(),
            "x86_64-unknown-linux-gnu"
                | "aarch64-unknown-linux-gnu"
                | "x86_64-unknown-linux-musl"
                | "aarch64-apple-darwin"
                | "x86_64-pc-windows-msvc"
        ),
        "Self-update is not published for target {target}"
    );
    let suffix = if cfg!(windows) { ".zip" } else { ".tar.gz" };
    let archive_name = format!("dm-{tag}-{target}{suffix}");
    let base = format!("https://github.com/{repository}/releases/download/{tag}");
    let archive = temp.path().join(&archive_name);
    let checksum = temp.path().join(format!("{archive_name}.sha256"));
    download(&format!("{base}/{archive_name}"), &archive)?;
    download(&format!("{base}/{archive_name}.sha256"), &checksum)?;
    verify_checksum(&fs::read(&archive)?, &fs::read(&checksum)?)?;
    let replacement = extract_binary(&archive, temp.path(), &tag, &target)?;
    info!("installing dm {available}");
    replace_current_executable(&replacement)?;
    Ok(SelfUpdateResult {
        updated: true,
        ..result
    })
}

pub fn cleanup_self_update_backup() -> Result<()> {
    #[cfg(windows)]
    {
        let backup = env::current_exe()?
            .canonicalize()?
            .with_extension("dm-previous.exe");
        if backup.exists() {
            fs::remove_file(backup)?;
        }
    }
    Ok(())
}

pub fn normalize_tag(version: &str) -> Result<String> {
    let value = version.strip_prefix('v').unwrap_or(version);
    let parsed = Version::parse(value)?;
    Ok(format!("v{parsed}"))
}

pub fn validate_repository(repository: &str) -> Result<()> {
    ensure!(
        repository.split('/').count() == 2
            && repository
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric()
                    || matches!(byte, b'/' | b'-' | b'_' | b'.')),
        "DM_UPDATE_REPOSITORY must be in owner/repository form"
    );
    Ok(())
}

fn download(url: &str, destination: &std::path::Path) -> Result<()> {
    info!("downloading {url}");
    let status = Command::new("curl")
        .args([
            "-fsSL",
            "--retry",
            "3",
            "--connect-timeout",
            "15",
            "--output",
        ])
        .arg(destination)
        .arg(url)
        .status()
        .context("Self-update requires curl")?;
    ensure!(status.success(), "Could not download {url}");
    Ok(())
}

pub fn verify_checksum(archive: &[u8], checksum: &[u8]) -> Result<()> {
    let text = std::str::from_utf8(checksum)?.trim();
    let expected = text
        .split_whitespace()
        .next()
        .context("Empty SHA-256 file")?;
    ensure!(
        expected.len() == 64 && expected.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "Invalid SHA-256 file"
    );
    let actual = format!("{:x}", Sha256::digest(archive));
    ensure!(
        actual.eq_ignore_ascii_case(expected),
        "Release SHA-256 mismatch"
    );
    Ok(())
}

fn replace_current_executable(replacement: &std::path::Path) -> Result<()> {
    let current = env::current_exe()?.canonicalize()?;
    let staged = current.with_extension("dm-update");
    if staged.exists() {
        fs::remove_file(&staged)?;
    }
    fs::copy(replacement, &staged)?;
    fs::set_permissions(&staged, fs::metadata(replacement)?.permissions())?;
    #[cfg(windows)]
    {
        let backup = current.with_extension("dm-previous.exe");
        if backup.exists() {
            let _ = fs::remove_file(&backup);
        }
        fs::rename(&current, &backup).context("Stage the current dm executable")?;
        if let Err(error) = fs::rename(&staged, &current) {
            let _ = fs::rename(&backup, &current);
            return Err(error).context("Install the new dm executable");
        }
    }
    #[cfg(not(windows))]
    fs::rename(&staged, &current).context("Install the new dm executable")?;
    Ok(())
}

fn extract_binary(
    archive: &std::path::Path,
    root: &std::path::Path,
    tag: &str,
    target: &str,
) -> Result<PathBuf> {
    #[cfg(windows)]
    let status = {
        Command::new("powershell")
            .args(["-NoProfile", "-Command"])
            .arg(format!(
                "Expand-Archive -LiteralPath '{}' -DestinationPath '{}' -Force",
                archive.display(),
                root.display()
            ))
            .status()
            .context("Self-update requires PowerShell to extract zip archives")?
    };
    #[cfg(not(windows))]
    let status = {
        Command::new("tar")
            .arg("-xf")
            .arg(archive)
            .arg("-C")
            .arg(root)
            .status()
            .context("Self-update requires tar")?
    };
    ensure!(status.success(), "Could not extract release archive");
    let binary = root
        .join(format!("dm-{tag}-{target}"))
        .join(if cfg!(windows) { "dm.exe" } else { "dm" });
    ensure!(
        fs::symlink_metadata(&binary)?.is_file(),
        "Release archive has no dm binary"
    );
    Ok(binary)
}
