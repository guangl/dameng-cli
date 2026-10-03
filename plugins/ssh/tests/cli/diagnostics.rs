use crate::common::*;
use tempfile::TempDir;

#[test]
fn doctor_needs_no_external_client_even_with_saved_passphrase() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let key = temp.path().join("key");
    std::fs::write(&key, "key fixture").unwrap();
    seed(
        &home,
        &[
            "add",
            "keyed",
            "--host",
            "example.invalid",
            "--username",
            "user",
            "--key",
            key.to_str().unwrap(),
            "--passphrase",
            "saved-phrase",
        ],
    );
    let report = ok(ssh(&home)
        .env("PATH", "")
        .args(["doctor", "--json"])
        .output()
        .unwrap());
    let json: serde_json::Value = serde_json::from_str(&report).unwrap();
    assert!(json["issues"].as_array().unwrap().is_empty());
    assert!(!report.contains("saved-phrase"));
}

#[test]
fn doctor_reports_key_path_problems_as_json() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    seed(
        &home,
        &[
            "add",
            "prod",
            "--host",
            "example.invalid",
            "--username",
            "root",
            "--key",
            "missing-key",
        ],
    );
    let output = ssh(&home).args(["doctor", "--json"]).output().unwrap();
    assert!(!output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        report["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|issue| issue.as_str().unwrap().contains("missing-key"))
    );
}
