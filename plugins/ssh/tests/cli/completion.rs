use crate::common::*;
use tempfile::TempDir;
#[test]
fn runtime_completion_supports_aliases_names_and_files_without_writes() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let output = ssh(&home).args(["__complete", ""]).output().unwrap();
    let candidates = ok(output.clone());
    assert!(candidates.lines().any(|v| v == "connect"));
    assert!(!candidates.lines().any(|v| v == "ssh"));
    assert!(output.stderr.is_empty());
    assert!(!home.exists());
    seed(
        &home,
        &[
            "add",
            "prod",
            "--host",
            "h",
            "--username",
            "root",
            "--password",
            "supersecret",
        ],
    );
    assert_eq!(
        ok(ssh(&home)
            .args(["__complete", "connect", "pr"])
            .output()
            .unwrap())
        .trim(),
        "prod"
    );
    let flags = ok(ssh(&home)
        .args(["__complete", "edit", "prod", "--"])
        .output()
        .unwrap());
    assert!(flags.contains("--key"));
    assert!(!flags.contains("supersecret"));
    let key = temp.path().join("private key");
    std::fs::write(&key, "secret").unwrap();
    let prefix = format!("{}/private", temp.path().display());
    assert_eq!(
        ok(ssh(&home)
            .args(["__complete", "add", "p", "--key", &prefix])
            .output()
            .unwrap())
        .trim(),
        format!("{prefix} key")
    );
}
