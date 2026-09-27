//! Prebuilt downloads are only accepted with a matching checksum.

use crate::common::*;
use sha2::{Digest, Sha256};
use std::fs;
use tempfile::TempDir;

#[cfg(unix)]
#[test]
fn prebuilt_checksum_mismatch_is_rejected() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    fs::remove_file(source.join(format!("dm-probe{}", std::env::consts::EXE_SUFFIX))).unwrap();
    let home = temp.path().join("home");
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

    let fake_bin = temp.path().join("fake-bin");
    fs::write(&fake_bin, "prebuilt-binary").unwrap();
    let curl = tools.join("curl");
    fs::write(
        &curl,
        r#"#!/bin/sh
out=""
url=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --output) out="$2"; shift 2;;
    -w) shift 2;;
    *) url="$1"; shift;;
  esac
done
case "$url" in
  *.sha256) printf '0000000000000000000000000000000000000000000000000000000000000000' > "$out";;
  *) cp "$FAKE_BIN" "$out";;
esac
printf 200
"#,
    )
    .unwrap();
    fs::set_permissions(&curl, fs::Permissions::from_mode(0o755)).unwrap();

    let path = std::env::join_paths(
        std::iter::once(tools).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let output = dm(&home)
        .env("PATH", &path)
        .env("FAKE_GIT_SOURCE", &source)
        .env("FAKE_BIN", &fake_bin)
        .args(["install", "https://github.com/example/probe.git"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("SHA-256 mismatch"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
#[cfg(unix)]
#[test]
fn prebuilt_checksum_match_is_accepted() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    fs::remove_file(source.join(format!("dm-probe{}", std::env::consts::EXE_SUFFIX))).unwrap();
    let home = temp.path().join("home");
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

    let fake_bin = temp.path().join("fake-bin");
    let bytes = b"prebuilt-binary";
    fs::write(&fake_bin, bytes).unwrap();
    let digest = format!("{:x}", Sha256::digest(bytes));
    let curl = tools.join("curl");
    fs::write(
        &curl,
        r#"#!/bin/sh
out=""
url=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --output) out="$2"; shift 2;;
    -w) shift 2;;
    *) url="$1"; shift;;
  esac
done
case "$url" in
  *.sha256) printf '%s' "__DIGEST__" > "$out";;
  *) cp "$FAKE_BIN" "$out";;
esac
printf 200
"#
        .replace("__DIGEST__", &digest),
    )
    .unwrap();
    fs::set_permissions(&curl, fs::Permissions::from_mode(0o755)).unwrap();

    let path = std::env::join_paths(
        std::iter::once(tools).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let output = dm(&home)
        .env("PATH", &path)
        .env("FAKE_GIT_SOURCE", &source)
        .env("FAKE_BIN", &fake_bin)
        .args(["install", "https://github.com/example/probe.git"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
