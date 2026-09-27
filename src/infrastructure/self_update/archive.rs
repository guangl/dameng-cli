use anyhow::{Context, Result, ensure};
use std::{env, fs, path::PathBuf, process::Command};

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

pub(super) fn replace_current_executable(replacement: &std::path::Path) -> Result<()> {
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

pub(super) fn extract_binary(
    archive: &std::path::Path,
    root: &std::path::Path,
    tag: &str,
    target: &str,
) -> Result<PathBuf> {
    #[cfg(windows)]
    let status = {
        Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "Expand-Archive -LiteralPath $env:DM_UPDATE_ARCHIVE -DestinationPath $env:DM_UPDATE_ROOT -Force",
            ])
            .env("DM_UPDATE_ARCHIVE", archive)
            .env("DM_UPDATE_ROOT", root)
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
