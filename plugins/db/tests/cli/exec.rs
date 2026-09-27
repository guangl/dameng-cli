//! End-to-end tests for "dm db exec".

use crate::common::*;
use std::{fs, io::Write, process::Stdio};
use tempfile::TempDir;

#[test]
fn exec_reads_sql_from_stdin_or_a_file() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    ok(db(&home)
        .args(["add", "prod", "--host", "10.0.0.8", "--password", "pw"])
        .output()
        .unwrap());

    // stdin is read before the driver is opened, so the statement is accepted
    // and only then is the deferred driver reported.
    let mut child = db(&home)
        .args(["exec", "prod"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"SELECT 1 FROM DUAL;")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    let stderr = failure(output);
    assert!(stderr.contains("driver is not implemented"), "{stderr}");

    // Empty stdin is rejected without touching the store.
    let mut child = db(&home)
        .args(["exec", "prod"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"   \n").unwrap();
    let stderr = failure(child.wait_with_output().unwrap());
    assert!(stderr.contains("SQL must not be empty"), "{stderr}");

    // A missing SQL file is reported as a file error.
    let missing = temp.path().join("missing.sql");
    let stderr = failure(
        db(&home)
            .args(["exec", "prod", "--file", missing.to_str().unwrap()])
            .output()
            .unwrap(),
    );
    assert!(stderr.contains("missing.sql"), "{stderr}");

    // Unknown connections are reported before any driver is loaded.
    let stderr = failure(
        db(&home)
            .args(["exec", "missing", "SELECT 1"])
            .output()
            .unwrap(),
    );
    assert!(stderr.contains("not configured"), "{stderr}");
}
