use dameng_cli::registry::parse_registry_index;
use dameng_cli::scaffold_plugin;
use dameng_cli::update::{
    normalize_tag, validate_repository, verify_checksum, verify_release_signature,
};
use sha2::{Digest, Sha256};

#[test]
fn scaffold_creates_a_complete_plugin_project_without_overwriting() {
    let temp = tempfile::tempdir().unwrap();
    let destination = temp.path().join("dm-plugin-backup");
    scaffold_plugin("backup", &destination).unwrap();
    assert!(destination.join("Cargo.toml").is_file());
    assert!(destination.join("dm-plugin.toml").is_file());
    assert!(destination.join("src/main.rs").is_file());
    assert!(scaffold_plugin("backup", &destination).is_err());
    assert!(scaffold_plugin("registry", &temp.path().join("bad")).is_err());
}

#[test]
fn update_helpers_validate_repository_and_versions() {
    assert!(validate_repository("guangl/dameng-cli").is_ok());
    assert!(validate_repository("https://example.com/x").is_err());
    assert_eq!(normalize_tag("0.2.0").unwrap(), "v0.2.0");
    assert_eq!(normalize_tag("v0.2.0").unwrap(), "v0.2.0");
}

#[test]
fn update_checksum_verification_rejects_tampering() {
    let digest = format!("{:x}  archive\n", Sha256::digest(b"archive"));
    assert!(verify_checksum(b"archive", digest.as_bytes()).is_ok());
    assert!(verify_checksum(b"changed", digest.as_bytes()).is_err());
}

#[test]
fn registry_index_parser_accepts_valid_entries_and_rejects_invalid() {
    let parsed = parse_registry_index(
        br#"[{"name":"backup","source":"https://example.invalid/backup.git"},{"name":"tools","source":"https://example.invalid/tools.git"}]"#,
    )
    .unwrap();
    assert_eq!(
        parsed,
        vec![
            (
                "backup".to_string(),
                "https://example.invalid/backup.git".to_string()
            ),
            (
                "tools".to_string(),
                "https://example.invalid/tools.git".to_string()
            ),
        ]
    );
    assert!(
        parse_registry_index(br#"[{"name":"bad_name","source":"https://example.invalid/x.git"}]"#)
            .is_err()
    );
    assert!(
        parse_registry_index(br#"[{"name":"backup","source":"http://example.invalid/x.git"}]"#)
            .is_err()
    );
}

#[test]
fn release_signature_verification_accepts_rsign_signature() {
    let signature = br#"untrusted comment: signature from rsign secret key
RUS7NJlQNVKoGOxn2EoqG2NHCN0enNX/Yd+1dkSpQzdMTrnucI/L8Kvh+jMccSYW7F0w0KekD0tP0Hz8rSVXg/JAW0KT3bUOWgg=
trusted comment: timestamp:1789964763	file:dm-archive.bin	prehashed
kc3x7cAU3ju8e0GV5ePI27dCKzf7jWkIii2UAPifVxgyAv07j7qXlG5lSyZ+P/HSJEcNUSXMsVhsmyU/qVBVAg=="#;
    if let Err(error) = verify_release_signature(b"archive", signature) {
        panic!("{error:#}");
    }
    assert!(verify_release_signature(b"changed", signature).is_err());
}
