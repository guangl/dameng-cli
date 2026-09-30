use crate::common::*;
use tempfile::TempDir;
#[test]
fn runtime_completion_reads_names_without_creating_a_store_or_exposing_passwords() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let output = db(&home).args(["__complete", ""]).output().unwrap();
    let candidates = ok(output.clone());
    assert!(candidates.lines().any(|v| v == "edit"));
    assert!(output.stderr.is_empty());
    assert!(!home.exists());
    ok(db(&home)
        .args(["add", "prod", "--host", "h", "--password", "supersecret"])
        .output()
        .unwrap());
    assert_eq!(
        ok(db(&home)
            .args(["__complete", "edit", "pr"])
            .output()
            .unwrap())
        .trim(),
        "prod"
    );
    let flags = ok(db(&home)
        .args(["__complete", "edit", "prod", "--"])
        .output()
        .unwrap());
    assert!(flags.contains("--port"));
    assert!(flags.contains("--clear-schema"));
    assert!(!flags.contains("supersecret"));
}
