use crate::common::*;
use std::io::Write;
use tempfile::TempDir;
#[path = "../support/server.rs"]
mod server;

#[test]
fn native_password_add_test_connect_and_failed_replace_without_external_tools() {
    let peer = server::Fixture::new();
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let port = peer.port.to_string();
    let args = [
        "add",
        "prod",
        "--host",
        "127.0.0.1",
        "--port",
        &port,
        "--username",
        "user",
        "--password",
        "fixture-password",
    ];
    let output = ok(ssh(&home).env("PATH", "").args(args).output().unwrap());
    assert!(output.contains("认证测试成功"));
    assert!(
        ok(ssh(&home)
            .env("PATH", "")
            .args(["test", "prod"])
            .output()
            .unwrap())
        .contains("测试成功")
    );
    let before = std::fs::read(home.join("data/ssh/servers.sqlite3")).unwrap();
    let mut wrong = args;
    wrong[9] = "wrong-secret";
    let error = failure(ssh(&home).args(wrong).arg("--replace").output().unwrap());
    assert!(error.contains("认证失败"), "{error}");
    assert!(error.contains("配置未保存"));
    assert!(!error.contains("wrong-secret"));
    assert_eq!(
        before,
        std::fs::read(home.join("data/ssh/servers.sqlite3")).unwrap()
    );
    let mut child = ssh(&home)
        .env("PATH", "")
        .args(["connect", "prod"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"native-shell-ok\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(7));
    assert!(String::from_utf8_lossy(&output.stdout).contains("native-shell-ok"));
    let rejected = failure(ssh(&home).args(["ssh", "prod"]).output().unwrap());
    assert!(rejected.contains("unrecognized subcommand"), "{rejected}");
}

#[test]
fn native_encrypted_private_key_authentication_and_wrong_passphrase() {
    let peer = server::Fixture::new();
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let path = temp.path().join("encrypted key");
    peer.key
        .encrypt(&mut rand::rng(), "key-phrase")
        .unwrap()
        .write_openssh_file(&path, russh::keys::ssh_key::LineEnding::LF)
        .unwrap();
    let port = peer.port.to_string();
    ok(ssh(&home)
        .env("PATH", "")
        .args([
            "add",
            "keyed",
            "--host",
            "127.0.0.1",
            "--port",
            &port,
            "--username",
            "user",
            "--key",
        ])
        .arg(&path)
        .args(["--passphrase", "key-phrase"])
        .output()
        .unwrap());
    ok(ssh(&home)
        .env("PATH", "")
        .args(["test", "keyed"])
        .output()
        .unwrap());
    let error = failure(
        ssh(&home)
            .args([
                "add",
                "bad",
                "--host",
                "127.0.0.1",
                "--port",
                &port,
                "--username",
                "user",
                "--key",
            ])
            .arg(&path)
            .args(["--passphrase", "wrong-phrase"])
            .output()
            .unwrap(),
    );
    assert!(error.contains("配置未保存"));
    assert!(!error.contains("wrong-phrase"));
    let entries = ok(ssh(&home).args(["list", "--json"]).output().unwrap());
    let entries: serde_json::Value = serde_json::from_str(&entries).unwrap();
    assert_eq!(entries.as_array().unwrap().len(), 1);
}

#[test]
fn changed_host_key_is_rejected_and_failed_first_auth_does_not_pin() {
    let peer = server::Fixture::new();
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let port = peer.port.to_string();
    let args = [
        "add",
        "prod",
        "--host",
        "127.0.0.1",
        "--port",
        &port,
        "--username",
        "user",
        "--password",
        "wrong-password",
    ];
    let error = failure(ssh(&home).args(args).output().unwrap());
    assert!(error.contains("配置未保存"));
    assert!(!home.join("data/ssh/known_hosts").exists());
    let mut good = args;
    good[9] = "fixture-password";
    ok(ssh(&home).args(good).output().unwrap());
    // Seed a different trusted host key for the same endpoint.
    std::fs::write(
        home.join("data/ssh/known_hosts"),
        format!(
            "[127.0.0.1]:{} {}\n",
            peer.port,
            peer.key.public_key().to_openssh().unwrap()
        ),
    )
    .unwrap();
    let error = failure(ssh(&home).args(["test", "prod"]).output().unwrap());
    assert!(error.contains("主机密钥校验失败"), "{error}");
}

#[test]
fn add_times_out_during_handshake_and_never_saves() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    write_plugin_config(&home, "[test]\nconnect_timeout = 1\n");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port().to_string();
    let start = std::time::Instant::now();
    let error = failure(
        ssh(&home)
            .args([
                "add",
                "silent",
                "--host",
                "127.0.0.1",
                "--port",
                &port,
                "--username",
                "user",
                "--password",
                "hidden-password",
            ])
            .output()
            .unwrap(),
    );
    assert!(error.contains("timed out"), "{error}");
    assert!(error.contains("配置未保存"));
    assert!(!error.contains("hidden-password"));
    assert!(start.elapsed() < std::time::Duration::from_secs(5));
    assert_eq!(
        ok(ssh(&home).args(["list", "--json"]).output().unwrap()).trim(),
        "[]"
    );
}
