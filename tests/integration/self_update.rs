use sha2::{Digest, Sha256};
use std::{fs, process::Command};
use tempfile::TempDir;

#[cfg(unix)]
#[test]
fn self_update_replaces_binary_from_signed_release() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempDir::new().unwrap();
    let bin = temp.path().join("bin");
    fs::create_dir(&bin).unwrap();
    let dm_path = bin.join("dm");
    fs::copy(env!("CARGO_BIN_EXE_dm"), &dm_path).unwrap();
    fs::set_permissions(&dm_path, fs::Permissions::from_mode(0o755)).unwrap();

    let archive_bytes = b"fake release archive";
    let digest = format!("{:x}  archive\n", Sha256::digest(archive_bytes));
    let keypair = minisign::KeyPair::generate_unencrypted_keypair().unwrap();
    let signature = minisign::sign(
        Some(&keypair.pk),
        &keypair.sk,
        std::io::Cursor::new(archive_bytes),
        None,
        None,
    )
    .unwrap()
    .to_string();

    let archive = temp.path().join("archive.tar.gz");
    fs::write(&archive, archive_bytes).unwrap();
    let checksum = temp.path().join("archive.sha256");
    fs::write(&checksum, digest.as_bytes()).unwrap();
    let minisig = temp.path().join("archive.minisig");
    fs::write(&minisig, signature.as_bytes()).unwrap();

    let tools = temp.path().join("tools");
    fs::create_dir(&tools).unwrap();
    let curl = tools.join("curl");
    fs::write(
        &curl,
        r#"#!/bin/sh
dest=""
url=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --output) dest="$2"; shift 2;;
    *) url="$1"; shift;;
  esac
done
case "$url" in
  *.minisig) cp "$FAKE_MINISIG" "$dest";;
  *.sha256) cp "$FAKE_SHA256" "$dest";;
  *) cp "$FAKE_ARCHIVE" "$dest";;
esac
"#,
    )
    .unwrap();
    fs::set_permissions(&curl, fs::Permissions::from_mode(0o755)).unwrap();

    let tar = tools.join("tar");
    fs::write(
        &tar,
        r#"#!/bin/sh
root=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    -C) root="$2"; shift 2;;
    *) shift;;
  esac
done
mkdir -p "$root/dm-$FAKE_TAG-$FAKE_TARGET"
printf 'new-dm-binary' > "$root/dm-$FAKE_TAG-$FAKE_TARGET/dm"
"#,
    )
    .unwrap();
    fs::set_permissions(&tar, fs::Permissions::from_mode(0o755)).unwrap();

    let path = std::env::join_paths(
        std::iter::once(tools).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();

    let output = Command::new(&dm_path)
        .env("PATH", &path)
        .env("DM_UPDATE_REPOSITORY", "example.invalid/repo")
        .env("DM_MINISIGN_PUBLIC_KEY", keypair.pk.to_base64())
        .env("FAKE_ARCHIVE", &archive)
        .env("FAKE_SHA256", &checksum)
        .env("FAKE_MINISIG", &minisig)
        .env("FAKE_TAG", "v0.1.0")
        .env("FAKE_TARGET", "aarch64-apple-darwin")
        .args([
            "self-update",
            "--force",
            "--version",
            "v0.1.0",
            "--target",
            "aarch64-apple-darwin",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read(&dm_path).unwrap(), b"new-dm-binary");
}

#[test]
fn self_update_check_reports_available_version_without_network() {
    let output = Command::new(env!("CARGO_BIN_EXE_dm"))
        .args(["self-update", "--check", "--version", "9.9.9", "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(parsed["updated"], false);
    assert_eq!(parsed["available_version"], "9.9.9");
}
