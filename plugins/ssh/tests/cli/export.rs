use std::fs;

use tempfile::TempDir;

use crate::common::*;

#[test]
fn export_and_import_round_trip_servers() {
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("source");
    fs::create_dir_all(&source).unwrap();
    seed(
        &source,
        &[
            "add",
            "prod",
            "--host",
            "10.0.0.8",
            "--port",
            "2222",
            "--username",
            "root",
            "--password",
            "p@ssw0rd",
        ],
    );
    seed(
        &source,
        &[
            "add",
            "keyed",
            "--host",
            "10.0.0.9",
            "--username",
            "ubuntu",
            "--key",
            "/tmp/id_ed25519",
            "--passphrase",
            "secret",
        ],
    );

    let stdout = ok(ssh(&source).args(["export"]).output().unwrap());
    assert!(stdout.contains("\"servers\""), "{stdout}");

    let file = temp.path().join("servers.json");
    let export = ok(ssh(&source)
        .args(["export", "--file"])
        .arg(&file)
        .output()
        .unwrap());
    assert!(export.contains("Exported 2 SSH servers"), "{export}");
    let json = fs::read_to_string(&file).unwrap();
    assert!(!json.contains("p@ssw0rd"), "{json}");
    assert!(!json.contains("\"secret\""), "{json}");

    let overwrite = failure(
        ssh(&source)
            .args(["export", "--file"])
            .arg(&file)
            .output()
            .unwrap(),
    );
    assert!(overwrite.contains("without overwriting"), "{overwrite}");

    let target = temp.path().join("target");
    fs::create_dir_all(&target).unwrap();
    let import = ok(ssh(&target).args(["import"]).arg(&file).output().unwrap());
    assert!(import.contains("Imported 2 SSH servers"), "{import}");
    let list = ok(ssh(&target).args(["list"]).output().unwrap());
    for needle in [
        "root",
        "10.0.0.8",
        "2222",
        "ubuntu",
        "10.0.0.9",
        "/tmp/id_ed25519",
    ] {
        assert!(list.contains(needle), "missing {needle:?} in:\n{list}");
    }

    let duplicate = failure(ssh(&target).args(["import"]).arg(&file).output().unwrap());
    assert!(duplicate.contains("--replace"), "{duplicate}");
    let replaced = ok(ssh(&target)
        .args(["import", "--replace"])
        .arg(&file)
        .output()
        .unwrap());
    assert!(replaced.contains("Imported 2 SSH servers"), "{replaced}");
}

#[test]
fn import_reports_missing_and_malformed_documents() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();

    let missing = failure(
        ssh(&home)
            .args(["import"])
            .arg(temp.path().join("missing.json"))
            .output()
            .unwrap(),
    );
    assert!(missing.contains("missing.json"), "{missing}");

    let broken = temp.path().join("broken.json");
    fs::write(&broken, "not json").unwrap();
    let malformed = failure(ssh(&home).args(["import"]).arg(&broken).output().unwrap());
    assert!(malformed.contains("Parse SSH server export"), "{malformed}");
    // The friendly hint names the passphrase and terminal requirements.
    assert!(malformed.contains("导入/导出"), "{malformed}");
}

#[test]
fn encrypted_export_needs_a_terminal() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    seed(
        &home,
        &[
            "add",
            "prod",
            "--host",
            "10.0.0.8",
            "--username",
            "root",
            "--password",
            "p@ssw0rd",
        ],
    );

    let file = temp.path().join("servers.json");
    let stderr = failure(
        ssh(&home)
            .args(["export", "--include-secrets", "--file"])
            .arg(&file)
            .output()
            .unwrap(),
    );
    assert!(stderr.contains("terminal"), "{stderr}");
    assert!(!file.exists());
}
