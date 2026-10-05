#![cfg(unix)]
use crate::{common::*, source_build_common::*};
use std::{fs, process::Command};
use tempfile::TempDir;

#[test]
fn fixed_toolchain_selection_and_explicit_install_do_not_change_defaults() {
    let temp = TempDir::new().unwrap();
    let source = source(temp.path());
    let tools = tools(temp.path());
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    write_config(&home, "[build]\ntoolchain='1.97.0'\n");
    fs::write(
        source.join("rust-toolchain.toml"),
        "[toolchain]\nchannel='1.98.0'\n",
    )
    .unwrap();
    ok(command(&home, &tools, temp.path())
        .args(["install", source.to_str().unwrap(), "--build"])
        .output()
        .unwrap());
    assert!(
        calls(temp.path()).contains("run 1.98.0 cargo build --release --locked --bin dm-probe")
    );
    assert!(!source.join("dm-probe").exists());
    assert!(!source.join("target").exists());
    ok(dm(&home).arg("probe").output().unwrap());
    fs::remove_file(temp.path().join("calls")).unwrap();
    ok(command(&home, &tools, temp.path())
        .env("MISSING_TOOLCHAIN", "1")
        .env("DM_BUILD_TOOLCHAIN", "invalid-unused")
        .args([
            "install",
            source.to_str().unwrap(),
            "--build",
            "--replace",
            "--toolchain",
            "1.99.0",
            "--install-toolchain",
        ])
        .output()
        .unwrap());
    let log = calls(temp.path());
    assert!(log.contains("toolchain install 1.99.0 --profile minimal"));
    assert!(log.contains("run 1.99.0 cargo build"));
    assert!(!log.contains("default"));
    fs::remove_file(source.join("rust-toolchain.toml")).unwrap();
    ok(command(&home, &tools, temp.path())
        .args(["install", source.to_str().unwrap(), "--build", "--replace"])
        .output()
        .unwrap());
    assert!(calls(temp.path()).contains("run 1.97.0 cargo build"));
    ok(command(&home, &tools, temp.path())
        .env("DM_BUILD_TOOLCHAIN", "1.96.0")
        .args(["install", source.to_str().unwrap(), "--build", "--replace"])
        .output()
        .unwrap());
    assert!(calls(temp.path()).contains("run 1.96.0 cargo build"));
}

#[test]
fn build_failure_and_conflicts_preserve_the_installed_plugin() {
    let temp = TempDir::new().unwrap();
    let source = source(temp.path());
    let tools = tools(temp.path());
    let home = temp.path().join("home");
    ok(command(&home, &tools, temp.path())
        .args(["install", source.to_str().unwrap(), "--build"])
        .output()
        .unwrap());
    fs::remove_file(temp.path().join("calls")).unwrap();
    let conflict = command(&home, &tools, temp.path())
        .args(["install", source.to_str().unwrap(), "--build"])
        .output()
        .unwrap();
    assert!(!conflict.status.success());
    assert!(calls(temp.path()).is_empty());
    for variable in ["BUILD_EXIT", "NO_BINARY"] {
        let output = command(&home, &tools, temp.path())
            .env(variable, "1")
            .args(["install", source.to_str().unwrap(), "--build", "--replace"])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(ok(dm(&home).arg("probe").output().unwrap()).contains("compiled plugin"));
    }
    let output = command(&home, &tools, temp.path())
        .env("MISSING_TOOLCHAIN", "1")
        .args(["install", source.to_str().unwrap(), "--build", "--replace"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(stderr(&output).contains("--install-toolchain"));
    assert!(stderr(&output).contains("提示：源码编译需要 rustup"));
    let output = command(&home, &tools, temp.path())
        .env("MISSING_TOOLCHAIN", "1")
        .env("INSTALL_EXIT", "1")
        .args([
            "install",
            source.to_str().unwrap(),
            "--build",
            "--replace",
            "--install-toolchain",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(stderr(&output).contains("Failed to install Rust"));
}

#[test]
fn real_cargo_build_uses_sources_and_enforces_locked_dependencies_and_msrv() {
    let temp = TempDir::new().unwrap();
    let source = source(temp.path());
    let tools = tools(temp.path());
    let home = temp.path().join("home");
    let real_rustup = Command::new("which").arg("rustup").output().unwrap();
    let rustup = String::from_utf8(real_rustup.stdout).unwrap();
    fs::write(source.join("dm-probe"), "old prebuilt").unwrap();
    ok(command(&home, &tools, temp.path())
        .env("REAL_BUILD", "1")
        .env("REAL_RUSTUP", rustup.trim())
        .args(["install", source.to_str().unwrap(), "--build"])
        .output()
        .unwrap());
    assert!(ok(dm(&home).arg("probe").output().unwrap()).contains("compiled plugin"));
    assert_eq!(
        fs::read_to_string(source.join("dm-probe")).unwrap(),
        "old prebuilt"
    );
    for changed in ["rust-version='1.100.0'", "version='0.2.0'"] {
        let original = fs::read_to_string(source.join("Cargo.toml")).unwrap();
        let text = if changed.starts_with("rust") {
            original.replace("rust-version='1.99.0'", changed)
        } else {
            original.replace("version='0.1.0'", changed)
        };
        fs::write(source.join("Cargo.toml"), text).unwrap();
        let output = command(&home, &tools, temp.path())
            .env("REAL_BUILD", "1")
            .env("REAL_RUSTUP", rustup.trim())
            .args(["install", source.to_str().unwrap(), "--build", "--replace"])
            .output()
            .unwrap();
        assert!(!output.status.success(), "{changed}");
        fs::write(source.join("Cargo.toml"), original).unwrap();
        assert!(ok(dm(&home).arg("probe").output().unwrap()).contains("compiled plugin"));
    }
}
