use crate::common::*;
use tempfile::TempDir;
#[test]
fn doctor_reports_configuration_and_key_path_problems_as_json() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    ok(ssh(&home)
        .args([
            "add",
            "prod",
            "--host",
            "example.invalid",
            "--username",
            "root",
            "--key",
            "missing-key",
        ])
        .output()
        .unwrap());
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
#[cfg(unix)]
#[test]
fn connection_failures_preserve_details_and_give_the_right_hint() {
    use std::{fs, os::unix::fs::PermissionsExt};
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let tools = temp.path().join("tools");
    fs::create_dir(&tools).unwrap();
    let executable = tools.join("sshpass");
    fs::write(
        &executable,
        "#!/bin/sh\necho 'Permission denied (password)' >&2\nexit 5\n",
    )
    .unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::join_paths(
        std::iter::once(tools).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    ok(ssh(&home)
        .args([
            "add",
            "prod",
            "--host",
            "example.invalid",
            "--username",
            "root",
            "--password",
            "secret",
        ])
        .output()
        .unwrap());
    let stderr = failure(
        ssh(&home)
            .env("PATH", &path)
            .args(["test"])
            .output()
            .unwrap(),
    );
    assert!(stderr.contains("Permission denied"));
    assert!(stderr.contains("认证失败"));
    assert!(!stderr.contains("secret"));
    fs::write(
        executable,
        "#!/bin/sh\necho 'Connection timed out' >&2\nexit 255\n",
    )
    .unwrap();
    let stderr = failure(
        ssh(&home)
            .env("PATH", &path)
            .args(["test", "prod"])
            .output()
            .unwrap(),
    );
    assert!(stderr.contains("网络连接失败"));
}
