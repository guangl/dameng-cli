use crate::common::*;
use tempfile::TempDir;
#[cfg(unix)]
#[test]
fn doctor_requires_sshpass_only_when_a_key_has_a_saved_passphrase() {
    use std::{fs, os::unix::fs::PermissionsExt};
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let tools = temp.path().join("tools");
    fs::create_dir(&tools).unwrap();
    let tool = tools.join("ssh");
    fs::write(&tool, "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&tool, fs::Permissions::from_mode(0o755)).unwrap();
    let key = temp.path().join("key");
    fs::write(&key, "test key fixture").unwrap();
    ok(ssh(&home)
        .args([
            "add",
            "keyed",
            "--host",
            "example.invalid",
            "--username",
            "user",
            "--key",
        ])
        .arg(&key)
        .output()
        .unwrap());
    for phrase in ["saved-phrase", ""] {
        ok(ssh(&home)
            .args(["edit", "keyed", "--passphrase", phrase])
            .output()
            .unwrap());
        let output = ssh(&home)
            .env("PATH", &tools)
            .args(["doctor", "--json"])
            .output()
            .unwrap();
        assert_eq!(output.status.success(), phrase.is_empty());
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            report["issues"]
                .as_array()
                .unwrap()
                .iter()
                .any(|issue| { issue.as_str().unwrap().contains("缺少 sshpass") }),
            !phrase.is_empty()
        );
        assert!(!String::from_utf8_lossy(&output.stdout).contains("saved-phrase"));
    }
}

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
