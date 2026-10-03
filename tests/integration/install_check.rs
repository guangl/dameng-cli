//! \`dm install --check\` previews an installation without changing the store.
//!
//! The fixtures are shell scripts, so these tests run on Unix only.
#![cfg(unix)]

use crate::common::*;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use tempfile::TempDir;

/// A package directory holding a manifest and a runnable `dm-probe`.
fn package(root: &std::path::Path) -> std::path::PathBuf {
    let source = root.join("package");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("dm-plugin.toml"), manifest("probe")).unwrap();
    let executable = source.join("dm-probe");
    fs::write(&executable, "#!/bin/sh\n").unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    source
}

#[test]
fn install_check_reports_the_installation_without_touching_the_store() {
    let temp = TempDir::new().unwrap();
    let source = package(temp.path());
    let home = temp.path().join("home");
    let source = source.to_str().unwrap();

    let checked = ok(dm(&home)
        .args(["install", source, "--check"])
        .output()
        .unwrap());
    assert!(checked.contains("可安装 probe 0.1.0"), "{checked}");
    assert!(checked.contains("首次安装"), "{checked}");
    assert!(checked.contains("来源包内的可执行文件"), "{checked}");
    // A check installs nothing: no plugins directory and no store record.
    assert!(!home.join("plugins").exists());
    assert!(ok(dm(&home).arg("list").output().unwrap()).contains("尚无插件"));

    // --dry-run is the same check and stays side-effect free.
    ok(dm(&home)
        .args(["install", source, "--dry-run"])
        .output()
        .unwrap());
    assert!(!home.join("plugins").exists());

    // Once installed, the check reports the same refusal as the installer.
    ok(dm(&home).args(["install", source]).output().unwrap());
    let refused = dm(&home)
        .args(["install", source, "--check"])
        .output()
        .unwrap();
    assert!(!refused.status.success());
    assert!(
        stderr(&refused).contains("already installed"),
        "{}",
        stderr(&refused)
    );

    // --replace previews an upgrade and still installs nothing.
    let replacing = ok(dm(&home)
        .args(["install", source, "--check", "--replace"])
        .output()
        .unwrap());
    assert!(replacing.contains("覆盖已安装插件"), "{replacing}");
    assert!(ok(dm(&home).arg("list").output().unwrap()).contains("probe"));
}

#[test]
fn install_check_fails_when_the_package_has_no_binary() {
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("package");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("dm-plugin.toml"), manifest("probe")).unwrap();
    let home = temp.path().join("home");

    let failure = dm(&home)
        .args(["install", source.to_str().unwrap(), "--check"])
        .output()
        .unwrap();
    assert!(!failure.status.success());
    assert!(
        stderr(&failure).contains("has no dm-probe binary"),
        "{}",
        stderr(&failure)
    );
    assert!(!home.join("plugins").join("probe").exists());
}
