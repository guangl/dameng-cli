//! `dm update` and `dm outdated` against local and Git sources.

use crate::common::*;
use dameng_cli::PluginStore;
use std::fs;
use tempfile::TempDir;

#[test]
fn outdated_command_reports_newer_local_version() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    ok(dm(&home)
        .args(["install", source.to_str().unwrap()])
        .output()
        .unwrap());
    assert!(ok(dm(&home).args(["outdated"]).output().unwrap()).contains("current"));

    fs::write(
        source.join("dm-plugin.toml"),
        manifest("probe").replace("0.1.0", "0.2.0"),
    )
    .unwrap();
    let output = ok(dm(&home).args(["outdated"]).output().unwrap());
    assert!(output.contains("0.2.0"));
    assert!(output.contains("update available"));
    let json = ok(dm(&home).args(["outdated", "--json"]).output().unwrap());
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed[0]["update_available"], true);
}
#[test]
fn update_all_command_updates_installed_plugins() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    ok(dm(&home)
        .args(["install", source.to_str().unwrap()])
        .output()
        .unwrap());
    let output = ok(dm(&home).args(["update", "--all"]).output().unwrap());
    assert!(output.contains("Updated probe to 0.1.0"), "{output}");
    assert!(home.join("plugins/probe/dm-plugin.toml").is_file());
}
#[cfg(unix)]
#[test]
fn update_https_source_checks_out_and_updates() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    let store = PluginStore::new(&home);
    store.install(source.to_str().unwrap()).unwrap();
    rusqlite::Connection::open(home.join("store.sqlite3"))
        .unwrap()
        .execute(
            "UPDATE installed_plugins SET source = ?1 WHERE name = 'probe'",
            ["https://github.com/example/probe.git"],
        )
        .unwrap();

    let tools = temp.path().join("tools");
    fs::create_dir(&tools).unwrap();
    let git = tools.join("git");
    fs::write(
        &git,
        r#"#!/bin/sh
last=""
for arg do last="$arg"; done
case " $* " in
  *" clone "*) cp -R "$FAKE_GIT_SOURCE" "$last";;
  *" rev-parse "*) printf '%040d
' 1;;
esac
"#,
    )
    .unwrap();
    fs::set_permissions(&git, fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::join_paths(
        std::iter::once(tools).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();

    let output = dm(&home)
        .env("PATH", &path)
        .env("FAKE_GIT_SOURCE", &source)
        .args(["update", "probe"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("Updated probe"));
}
#[test]
fn outdated_without_source_reports_no_available_version() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    let store = PluginStore::new(&home);
    store.install(source.to_str().unwrap()).unwrap();
    rusqlite::Connection::open(home.join("store.sqlite3"))
        .unwrap()
        .execute(
            "UPDATE installed_plugins SET source = NULL WHERE name = 'probe'",
            [],
        )
        .unwrap();

    let statuses = store.outdated().unwrap();
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].available_version, None);
    assert!(!statuses[0].update_available);
}
#[test]
fn outdated_with_removed_local_source_is_unknown() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    let store = PluginStore::new(&home);
    store.install(source.to_str().unwrap()).unwrap();
    // Installers unpack plugins into a temporary directory that is gone later.
    fs::remove_dir_all(&source).unwrap();

    let statuses = store.outdated().unwrap();
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].available_version, None);
    assert!(!statuses[0].update_available);

    let output = ok(dm(&home).args(["outdated"]).output().unwrap());
    assert!(output.contains("unknown"), "{output}");
}
#[cfg(unix)]
#[test]
fn outdated_https_source_uses_fake_git() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    let store = PluginStore::new(&home);
    store.install(source.to_str().unwrap()).unwrap();
    rusqlite::Connection::open(home.join("store.sqlite3"))
        .unwrap()
        .execute(
            "UPDATE installed_plugins SET source = ?1 WHERE name = 'probe'",
            ["https://github.com/example/probe.git"],
        )
        .unwrap();

    let tools = temp.path().join("tools");
    fs::create_dir(&tools).unwrap();
    let git = tools.join("git");
    fs::write(
        &git,
        r#"#!/bin/sh
last=""
for arg do last="$arg"; done
case " $* " in
  *" clone "*) cp -R "$FAKE_GIT_SOURCE" "$last";;
  *" rev-parse "*) printf '%040d
' 1;;
esac
"#,
    )
    .unwrap();
    fs::set_permissions(&git, fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::join_paths(
        std::iter::once(tools).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();

    let output = dm(&home)
        .env("PATH", &path)
        .env("FAKE_GIT_SOURCE", &source)
        .args(["outdated"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("0.1.0"));
}
