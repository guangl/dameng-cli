#![cfg(unix)]
use crate::{common::*, source_build_common::*};
use std::{fs, os::unix::fs::PermissionsExt};
use tempfile::TempDir;

#[test]
fn remote_source_build_records_revision_without_requiring_release_assets() {
    let temp = TempDir::new().unwrap();
    let source = source(temp.path());
    let tools = tools(temp.path());
    let home = temp.path().join("home");
    let git = tools.join("git");
    fs::write(
        &git,
        r#"#!/bin/sh
last=""
for arg do last="$arg"; done
case " $* " in
 *" clone "*) cp -R "$FAKE_GIT_SOURCE" "$last";;
 *" checkout "*) exit 0;;
 *" rev-parse "*) printf '%040d\n' 1;;
esac
"#,
    )
    .unwrap();
    fs::set_permissions(git, fs::Permissions::from_mode(0o755)).unwrap();
    ok(command(&home, &tools, temp.path())
        .env("FAKE_GIT_SOURCE", &source)
        .args([
            "install",
            "https://example.invalid/probe.git",
            "--build",
            "--rev",
            "v1.2.0",
        ])
        .output()
        .unwrap());
    let info = ok(dm(&home)
        .args(["info", "probe", "--json"])
        .output()
        .unwrap());
    let info: serde_json::Value = serde_json::from_str(&info).unwrap();
    assert_eq!(info["source"], "https://example.invalid/probe.git");
    assert_eq!(info["revision"], format!("{:040}", 1));
    assert_eq!(info["source_ref"], "v1.2.0");
    assert!(ok(dm(&home).arg("probe").output().unwrap()).contains("compiled plugin"));
}

#[test]
fn source_build_refuses_a_source_containing_the_plugin_store() {
    let temp = TempDir::new().unwrap();
    let source = source(temp.path());
    let tools = tools(temp.path());
    let home = source.join("home");
    fs::create_dir_all(home.join("plugins")).unwrap();
    let output = command(&home, &tools, temp.path())
        .args(["install", source.to_str().unwrap(), "--build"])
        .output()
        .unwrap();
    assert!(stderr(&output).contains("must not contain the plugin store"));
    assert!(calls(temp.path()).is_empty());
}
