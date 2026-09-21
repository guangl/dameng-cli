use sha2::{Digest, Sha256};
use std::{fs, process::Command};
use tempfile::TempDir;

fn write_fake_curl(tools: &std::path::Path) {
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
  *latest) printf '{"tag_name":"v9.9.9"}' > "$dest";;
  *) cp "$FAKE_ARCHIVE" "$dest";;
esac
"#,
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&curl, fs::Permissions::from_mode(0o755)).unwrap();
    }
}

fn write_fake_tar(tools: &std::path::Path) {
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
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tar, fs::Permissions::from_mode(0o755)).unwrap();
    }
}

fn prepend_tools_to_path(tools: &std::path::Path) -> std::ffi::OsString {
    std::env::join_paths(
        std::iter::once(tools.to_path_buf())
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap()
}

#[cfg(unix)]
#[test]
fn self_update_replaces_binary_from_signed_release() {
    use std::os::unix::fs::PermissionsExt;

    let keypair = minisign::KeyPair::generate_unencrypted_keypair().unwrap();
    for (index, target) in ["aarch64-apple-darwin", "x86_64-pc-windows-msvc"]
        .iter()
        .enumerate()
    {
        let temp = TempDir::new().unwrap();
        let bin = temp.path().join("bin");
        fs::create_dir(&bin).unwrap();
        let dm_path = bin.join("dm");
        fs::copy(env!("CARGO_BIN_EXE_dm"), &dm_path).unwrap();
        fs::set_permissions(&dm_path, fs::Permissions::from_mode(0o755)).unwrap();
        if index == 0 {
            fs::write(bin.join("dm.dm-update"), b"stale staged binary").unwrap();
        }

        let archive_bytes = b"fake release archive";
        let digest = format!("{:x}  archive\n", Sha256::digest(archive_bytes));
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
        write_fake_curl(&tools);
        write_fake_tar(&tools);

        let output = Command::new(&dm_path)
            .env("PATH", prepend_tools_to_path(&tools))
            .env("DM_UPDATE_REPOSITORY", "example.invalid/repo")
            .env("DM_MINISIGN_PUBLIC_KEY", keypair.pk.to_base64())
            .env("FAKE_ARCHIVE", &archive)
            .env("FAKE_SHA256", &checksum)
            .env("FAKE_MINISIG", &minisig)
            .env("FAKE_TAG", "v0.1.0")
            .env("FAKE_TARGET", target)
            .args([
                "self-update",
                "--force",
                "--version",
                "v0.1.0",
                "--target",
                target,
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

#[cfg(unix)]
#[test]
fn self_update_check_latest_uses_release_json() {
    let temp = TempDir::new().unwrap();
    let tools = temp.path().join("tools");
    fs::create_dir(&tools).unwrap();
    write_fake_curl(&tools);

    let output = Command::new(env!("CARGO_BIN_EXE_dm"))
        .env("PATH", prepend_tools_to_path(&tools))
        .env("DM_UPDATE_REPOSITORY", "example.invalid/repo")
        .args(["self-update", "--check", "--json"])
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

#[test]
fn self_update_wrapper_reports_up_to_date_without_network() {
    let result = dameng_cli::self_update(Some("0.1.0"), false).unwrap();
    assert!(!result.updated);
    assert_eq!(result.available_version, "0.1.0");
    assert_eq!(result.current_version, env!("CARGO_PKG_VERSION"));
}

#[test]
fn self_update_check_non_json_reports_current_and_available() {
    let current = Command::new(env!("CARGO_BIN_EXE_dm"))
        .args(["self-update", "--check", "--version", "0.2.0"])
        .output()
        .unwrap();
    assert!(
        current.status.success(),
        "{}",
        String::from_utf8_lossy(&current.stderr)
    );
    assert!(
        String::from_utf8_lossy(&current.stdout).contains("dm 0.2.0 is current"),
        "{}",
        String::from_utf8_lossy(&current.stdout)
    );

    let available = Command::new(env!("CARGO_BIN_EXE_dm"))
        .args(["self-update", "--check", "--version", "0.1.0"])
        .output()
        .unwrap();
    assert!(
        available.status.success(),
        "{}",
        String::from_utf8_lossy(&available.stderr)
    );
    let stdout = String::from_utf8_lossy(&available.stdout);
    assert!(stdout.contains("dm 0.2.0 is installed"), "{stdout}");
    assert!(stdout.contains("0.1.0 is available"), "{stdout}");
}
