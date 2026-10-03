//! Self-update helpers, checksums and repository parsing.

use dameng_cli::self_update::{normalize_tag, validate_repository, verify_checksum};
use dameng_cli::{github_repository, prebuilt_target_label_for, versions_differ};
use sha2::{Digest, Sha256};

#[test]
fn update_helpers_validate_repository_and_versions() {
    assert!(validate_repository("guangl/dameng-cli").is_ok());
    assert!(validate_repository("https://example.com/x").is_err());
    assert_eq!(normalize_tag("0.2.0").unwrap(), "v0.2.0");
    assert_eq!(normalize_tag("v0.2.0").unwrap(), "v0.2.0");
}
#[test]
fn update_checksum_verification_rejects_tampering() {
    let digest = format!(
        "{}  archive\n",
        dameng_cli::support::codec::hex(&Sha256::digest(b"archive"))
    );
    assert!(verify_checksum(b"archive", digest.as_bytes()).is_ok());
    assert!(verify_checksum(b"changed", digest.as_bytes()).is_err());
}
#[test]
fn github_repository_parses_valid_sources() {
    assert_eq!(
        github_repository("https://github.com/guangl/dameng-cli"),
        Some(("guangl", "dameng-cli"))
    );
    assert_eq!(
        github_repository("https://github.com/guangl/dameng-cli.git"),
        Some(("guangl", "dameng-cli"))
    );
    assert_eq!(github_repository("https://example.com/a/b"), None);
    assert_eq!(github_repository("https://github.com/a/b/c"), None);
}
#[test]
fn prebuilt_target_label_maps_all_targets() {
    assert_eq!(
        prebuilt_target_label_for("aarch64-apple-darwin"),
        Some("aarch64-macos")
    );
    assert_eq!(
        prebuilt_target_label_for("x86_64-apple-darwin"),
        Some("x86_64-macos")
    );
    assert_eq!(
        prebuilt_target_label_for("x86_64-unknown-linux-gnu"),
        Some("x86_64-linux")
    );
    assert_eq!(
        prebuilt_target_label_for("x86_64-unknown-linux-musl"),
        Some("x86_64-linux")
    );
    assert_eq!(
        prebuilt_target_label_for("aarch64-unknown-linux-gnu"),
        Some("aarch64-linux")
    );
    assert_eq!(
        prebuilt_target_label_for("aarch64-unknown-linux-musl"),
        Some("aarch64-linux")
    );
    assert_eq!(
        prebuilt_target_label_for("armv7-unknown-linux-gnueabihf"),
        Some("armv7-linux")
    );
    assert_eq!(
        prebuilt_target_label_for("x86_64-pc-windows-msvc"),
        Some("x86_64-windows")
    );
    assert_eq!(
        prebuilt_target_label_for("aarch64-pc-windows-msvc"),
        Some("aarch64-windows")
    );
    assert_eq!(prebuilt_target_label_for("unknown-target"), None);
}
#[test]
fn versions_differ_falls_back_to_equality() {
    assert!(versions_differ("0.1.0", "0.2.0"));
    assert!(!versions_differ("0.2.0", "0.1.0"));
    assert!(versions_differ("abc", "def"));
    assert!(!versions_differ("abc", "abc"));
}

#[test]
fn archive_checksums_stream_large_files_and_bound_sidecars() {
    use dameng_cli::self_update::verify_checksum_file;
    use std::io::Write;
    let temp = tempfile::tempdir().unwrap();
    let archive = temp.path().join("archive");
    let checksum = temp.path().join("checksum");
    let mut file = std::fs::File::create(&archive).unwrap();
    let mut hasher = Sha256::new();
    let block = [0x55; 64 * 1024];
    for _ in 0..256 {
        file.write_all(&block).unwrap();
        hasher.update(block);
    }
    drop(file);
    std::fs::write(
        &checksum,
        format!(
            "{}  archive",
            dameng_cli::support::codec::hex(&hasher.finalize())
        ),
    )
    .unwrap();
    verify_checksum_file(&archive, &checksum).unwrap();
    std::fs::write(&archive, b"tampered").unwrap();
    assert!(verify_checksum_file(&archive, &checksum).is_err());
    std::fs::write(&checksum, [b'x'; 4097]).unwrap();
    assert!(verify_checksum_file(&archive, &checksum).is_err());
    std::fs::write(&checksum, "invalid").unwrap();
    assert!(verify_checksum_file(&archive, &checksum).is_err());
    std::fs::write(&checksum, "").unwrap();
    assert!(verify_checksum_file(&archive, &checksum).is_err());
    std::fs::write(&checksum, [0xff]).unwrap();
    assert!(verify_checksum_file(&archive, &checksum).is_err());
    assert!(verify_checksum_file(&archive, &temp.path().join("missing")).is_err());
    std::fs::write(
        &checksum,
        dameng_cli::support::codec::hex(&Sha256::digest(b"")),
    )
    .unwrap();
    assert!(verify_checksum_file(&temp.path().join("missing"), &checksum).is_err());
}
