//! Persistent sources for plugin archives published alongside the host.
use super::{InstallMode, PluginStore, github_repository};
use crate::Manifest;
use crate::infrastructure::self_update::{normalize_tag, verify_checksum_file};
use crate::support::process::capture;
use anyhow::{Context, Result, ensure};
use std::time::Duration;
use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
};

pub(super) fn repository(source: &str) -> Option<&str> {
    source.strip_prefix("github-release:")
}
fn validate(repository: &str) -> Result<()> {
    ensure!(
        github_repository(&format!("https://github.com/{repository}")).is_some(),
        "Release source must use owner/repository form"
    );
    ensure!(
        !repository.split('/').any(|part| matches!(part, "." | "..")),
        "Invalid release repository"
    );
    Ok(())
}
pub(super) fn latest_tag(repository: &str) -> Result<String> {
    validate(repository)?;
    let output = capture(
        Command::new("curl")
            .args([
                "-q",
                "-fsSIL",
                "--connect-timeout",
                "10",
                "--max-time",
                "30",
                "-o",
                if cfg!(windows) { "NUL" } else { "/dev/null" },
                "-w",
                "%{url_effective}",
            ])
            .arg(format!("https://github.com/{repository}/releases/latest")),
        Duration::from_secs(180),
    )
    .context("查询最新 Release 需要 curl")?;
    ensure!(
        output.status.success(),
        "无法查询最新 Release：{}",
        String::from_utf8_lossy(&output.stderr)
    );
    normalize_tag(
        String::from_utf8_lossy(&output.stdout)
            .trim()
            .rsplit('/')
            .next()
            .context("Release tag 缺失")?,
    )
}
fn download(url: &str, destination: &Path) -> Result<()> {
    let output = capture(
        Command::new("curl")
            .args([
                "-q",
                "-fsSL",
                "--connect-timeout",
                "10",
                "--max-time",
                "120",
                "--output",
            ])
            .arg(destination)
            .arg(url),
        Duration::from_secs(180),
    )
    .context("下载 Release 需要 curl")?;
    ensure!(
        output.status.success(),
        "Release 下载失败：{}：{}",
        url,
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}
/// Read exactly one declared archive member; never extract arbitrary paths.
fn extract_file(archive: &Path, entry: &str, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination.parent().context("包目录无效")?)?;
    #[cfg(not(windows))]
    let output = Command::new("tar")
        .args(["-xOf"])
        .arg(archive)
        .arg(entry)
        .stdout(fs::File::create(destination)?)
        .stderr(Stdio::piped())
        .output()
        .context("读取 Release 包需要 tar")?;
    #[cfg(windows)]
    let output = Command::new("powershell").args(["-NoProfile", "-Command", r#"$ErrorActionPreference='Stop'; Add-Type -AssemblyName System.IO.Compression.FileSystem; $zip=[IO.Compression.ZipFile]::OpenRead($env:DM_PACKAGE_ARCHIVE); try { $entry=$zip.GetEntry($env:DM_PACKAGE_ENTRY); if ($null -eq $entry) { throw 'Missing archive entry' }; $archiveInput=$entry.Open(); $archiveOutput=[IO.File]::Create($env:DM_PACKAGE_DESTINATION); try { $archiveInput.CopyTo($archiveOutput) } finally { $archiveOutput.Dispose(); $archiveInput.Dispose() } } finally { $zip.Dispose() }"#])
        .env("DM_PACKAGE_ARCHIVE", archive).env("DM_PACKAGE_ENTRY", entry).env("DM_PACKAGE_DESTINATION", destination)
        .stdout(Stdio::null()).output().context("读取 Release 包需要 PowerShell")?;
    ensure!(
        output.status.success(),
        "Release 包缺少 {entry}：{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}
pub(super) fn package(
    repository: &str,
    name: &str,
    target: &str,
    tag: &str,
) -> Result<(tempfile::TempDir, std::path::PathBuf)> {
    validate(repository)?;
    crate::plugin::manifest::validate_name(name)?;
    let tag = normalize_tag(tag)?;
    ensure!(
        crate::SUPPORTED_TARGETS.contains(&target),
        "Unsupported release target {target}"
    );
    let temp = tempfile::tempdir()?;
    let folder = format!("dm-{name}-{tag}-{target}");
    let asset = format!(
        "{folder}{}",
        if target.ends_with("windows-msvc") {
            ".zip"
        } else {
            ".tar.gz"
        }
    );
    let archive = temp.path().join(&asset);
    let checksum = temp.path().join("checksum");
    let url = format!("https://github.com/{repository}/releases/download/{tag}/{asset}");
    download(&url, &archive)?;
    download(&format!("{url}.sha256"), &checksum)?;
    verify_checksum_file(&archive, &checksum)?;
    let root = temp.path().join("package");
    extract_file(
        &archive,
        &format!("{folder}/dm-plugin.toml"),
        &root.join("dm-plugin.toml"),
    )?;
    let manifest = Manifest::read(&root)?;
    ensure!(manifest.name == name, "Release plugin name mismatch");
    for file in std::iter::once(manifest.executable_name()).chain(
        [
            manifest.hooks.pre_install.clone(),
            manifest.hooks.post_install.clone(),
            manifest.hooks.pre_uninstall.clone(),
            manifest.hooks.post_uninstall.clone(),
        ]
        .into_iter()
        .flatten(),
    ) {
        extract_file(&archive, &format!("{folder}/{file}"), &root.join(&file))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(root.join(&file), fs::Permissions::from_mode(0o755))?;
        }
    }
    Ok((temp, root))
}
impl PluginStore {
    /// Install a local package with a persistent Release update source.
    pub fn install_release_package(
        &self,
        path: &Path,
        repository: &str,
        tag: &str,
        target: Option<&str>,
        replace: bool,
    ) -> Result<Manifest> {
        validate(repository)?;
        let tag = normalize_tag(tag)?;
        let target = target.unwrap_or(env!("DM_HOST_TARGET"));
        ensure!(
            crate::SUPPORTED_TARGETS.contains(&target),
            "Unsupported release target {target}"
        );
        self.install_directory(
            path,
            Some(format!("github-release:{repository}")),
            Some(tag),
            Some(target.into()),
            if replace {
                InstallMode::Replace
            } else {
                InstallMode::New
            },
        )
    }
}
