use crate::common::*;
use clap::Parser;
use dameng_cli::cli::Cli;
use tempfile::TempDir;

#[test]
fn source_build_flags_reject_incompatible_modes() {
    for args in [
        vec!["dm", "install", ".", "--toolchain", "1.99.0"],
        vec!["dm", "install", ".", "--install-toolchain"],
        vec!["dm", "install", ".", "--build", "--check"],
        vec![
            "dm",
            "install",
            ".",
            "--build",
            "--release-source",
            "owner/repo",
            "--release-tag",
            "v1",
        ],
    ] {
        assert!(Cli::try_parse_from(args).is_err());
    }
}

#[test]
fn config_build_toolchain_is_fixed_and_reported_with_its_source() {
    for value in [
        "stable",
        "nightly-2026-01-01",
        "/tmp/rust",
        "1.99",
        "1.99.0-beta",
        "1.99.0+custom",
        "",
        "2.0.0",
    ] {
        assert!(dameng_cli::Config::from_toml(&format!("[build]\ntoolchain={value:?}\n")).is_err());
    }
    let temp = TempDir::new().unwrap();
    write_config(temp.path(), "[build]\ntoolchain='1.98.0'\n");
    for (override_value, expected, origin) in [
        (None, "1.98.0", "config"),
        (Some("1.99.0"), "1.99.0", "env:DM_BUILD_TOOLCHAIN"),
    ] {
        let mut cmd = dm(temp.path());
        cmd.env_remove("DM_BUILD_TOOLCHAIN");
        if let Some(value) = override_value {
            cmd.env("DM_BUILD_TOOLCHAIN", value);
        }
        let output = ok(cmd.args(["config", "show", "--json"]).output().unwrap());
        let value: serde_json::Value = serde_json::from_str(&output).unwrap();
        let setting = value["settings"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["key"] == "build.toolchain")
            .unwrap();
        assert_eq!(setting["value"], expected);
        assert_eq!(setting["source"], origin);
    }
}

#[cfg(unix)]
#[test]
fn invalid_source_builds_fail_before_running_the_toolchain() {
    use crate::source_build_common::*;
    use std::fs;
    let temp = TempDir::new().unwrap();
    let source = source(temp.path());
    let tools = tools(temp.path());
    let home = temp.path().join("home");
    for text in [
        "[toolchain]\nchannel='stable'",
        "[toolchain]\npath='/tmp/custom'",
        "[toolchain]\nprofile='minimal'",
        "bad toml [",
        "channel='1.99.0'",
    ] {
        fs::write(source.join("rust-toolchain.toml"), text).unwrap();
        let output = command(&home, &tools, temp.path())
            .args(["install", source.to_str().unwrap(), "--build"])
            .output()
            .unwrap();
        assert!(!output.status.success(), "{text}");
        assert!(calls(temp.path()).is_empty());
    }
    fs::remove_file(source.join("rust-toolchain.toml")).unwrap();
    fs::write(source.join("rust-toolchain"), "stable").unwrap();
    let output = command(&home, &tools, temp.path())
        .args(["install", source.to_str().unwrap(), "--build"])
        .output()
        .unwrap();
    assert!(stderr(&output).contains("Legacy rust-toolchain"));
    fs::remove_file(source.join("rust-toolchain")).unwrap();
    for name in ["Cargo.lock", "Cargo.toml"] {
        fs::remove_file(source.join(name)).unwrap();
        let output = command(&home, &tools, temp.path())
            .args(["install", source.to_str().unwrap(), "--build"])
            .output()
            .unwrap();
        assert!(stderr(&output).contains(name));
        assert!(calls(temp.path()).is_empty());
    }
}
