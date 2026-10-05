//! Compatibility for the standalone sqllog2db release before its SDK entrypoint.
#![cfg(unix)]

use crate::common::*;
use sha2::{Digest, Sha256};
use std::{fs, os::unix::fs::PermissionsExt};

fn install(repository: &str, version: &str, native: bool, checksum: &str) -> (bool, String) {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    let tools = temp.path().join("tools");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&tools).unwrap();
    fs::write(
        source.join("dm-plugin.toml"),
        format!(
            "name = 'sqllog2db'\nversion = '{version}'\ndescription = 'probe'\napi_version = 1\n"
        ),
    )
    .unwrap();
    let binary = temp.path().join("binary");
    fs::write(&binary, "#!/bin/sh\nprintf '%s\\n' \"$@\"\n").unwrap();
    let requests = temp.path().join("requests");
    let git = tools.join("git");
    fs::write(
        &git,
        r#"#!/bin/sh
for destination do :; done
case " $* " in
  *" clone "*) cp -R "$FAKE_SOURCE" "$destination";;
  *" rev-parse "*) printf '%040d\n' 1;;
esac
"#,
    )
    .unwrap();
    let curl = tools.join("curl");
    fs::write(
        &curl,
        r#"#!/bin/sh
while [ "$#" -gt 0 ]; do
  case "$1" in
    --output) out="$2"; shift 2;;
    -w) shift 2;;
    *) url="$1"; shift;;
  esac
done
printf '%s\n' "$url" >> "$REQUESTS"
case "$url" in
  *.sha256)
    if [ "$CHECKSUM" = missing ]; then printf 404;
    else printf '%s' "$CHECKSUM" > "$out"; printf 200; fi;;
  */dm-sqllog2db-*)
    if [ "$NATIVE" = true ]; then cp "$FAKE_BINARY" "$out"; else exit 22; fi;;
  */sqllog2db-*) cp "$FAKE_BINARY" "$out";;
  *) exit 22;;
esac
"#,
    )
    .unwrap();
    for tool in [&git, &curl] {
        fs::set_permissions(tool, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let path = std::env::join_paths(
        std::iter::once(tools).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let home = temp.path().join("home");
    let output = dm(&home)
        .env("PATH", path)
        .env("FAKE_SOURCE", source)
        .env("FAKE_BINARY", &binary)
        .env("REQUESTS", &requests)
        .env("NATIVE", native.to_string())
        .env(
            "CHECKSUM",
            if checksum == "valid" {
                dameng_cli::support::codec::hex(&Sha256::digest(fs::read(&binary).unwrap()))
            } else {
                checksum.to_owned()
            },
        )
        .args(["install", &format!("https://github.com/{repository}.git")])
        .output()
        .unwrap();
    if output.status.success() {
        let arguments = ok(dm(&home)
            .args(["sqllog2db", "hello", "two words"])
            .output()
            .unwrap());
        assert_eq!(arguments, "hello\ntwo words\n");
        ok(dm(&home).args(["info", "sqllog2db"]).output().unwrap());
    } else {
        assert!(!home.join("plugins/sqllog2db").exists());
    }
    (
        output.status.success(),
        fs::read_to_string(requests).unwrap(),
    )
}

#[test]
fn legacy_sqllog2db_release_installs_and_runs() {
    for checksum in ["missing", "valid"] {
        let (success, requests) = install("guangl/dm-database-sqllog2db", "3.0.1", false, checksum);
        assert!(success);
        let urls: Vec<_> = requests.lines().collect();
        assert!(urls[0].contains("/dm-sqllog2db-"));
        assert!(urls[1].contains("/sqllog2db-"));
        assert_eq!(urls.len(), 2);
    }
}

#[test]
fn native_sqllog2db_asset_has_priority() {
    let (success, requests) = install("guangl/dm-database-sqllog2db", "3.0.1", true, "valid");
    assert!(success);
    assert!(!requests.contains("/sqllog2db-"));
}

#[test]
fn legacy_sqllog2db_never_requests_checksum_attachments() {
    let (success, requests) = install(
        "guangl/dm-database-sqllog2db",
        "3.0.1",
        false,
        &"0".repeat(64),
    );
    assert!(success);
    assert!(!requests.contains(".sha256"));
}

#[test]
fn standalone_fallback_is_limited_to_the_known_release() {
    for (repository, version) in [
        ("someone/dm-database-sqllog2db", "3.0.1"),
        ("guangl/another", "3.0.1"),
        ("guangl/dm-database-sqllog2db", "3.0.2"),
    ] {
        let (success, requests) = install(repository, version, false, "missing");
        assert!(!success);
        assert!(!requests.contains("/sqllog2db-"));
    }
}

#[test]
fn legacy_sqllog2db_repository_matching_is_case_insensitive() {
    assert!(install("Guangl/DM-Database-Sqllog2db", "3.0.1", false, "valid").0);
}
