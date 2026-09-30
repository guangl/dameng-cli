use std::fs;

use tempfile::TempDir;

use crate::common::*;

#[test]
fn add_list_and_remove_servers() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();

    let add = ok(ssh(&home)
        .args([
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
        ])
        .output()
        .unwrap());
    assert!(add.contains("已保存 SSH 连接 prod"), "{add}");

    let list = ok(ssh(&home).args(["list"]).output().unwrap());
    for needle in [
        "名称",
        "地址",
        "端口",
        "用户名",
        "认证方式",
        "私钥",
        "prod",
        "10.0.0.8",
        "2222",
        "root",
        "password",
    ] {
        assert!(list.contains(needle), "missing {needle:?} in:\n{list}");
    }
    assert!(!list.contains("p@ssw0rd"), "{list}");
    assert!(list.ends_with('\n'), "the table must end with a newline");

    let remove = ok(ssh(&home)
        .args(["remove", "prod", "--yes"])
        .output()
        .unwrap());
    assert!(remove.contains("已删除 SSH 连接 prod"), "{remove}");
    let empty = ok(ssh(&home).args(["list"]).output().unwrap());
    assert!(empty.contains("dm ssh add <name>"), "{empty}");
}

#[test]
fn list_json_reports_servers_without_secrets() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();

    let empty = ok(ssh(&home).args(["list", "--json"]).output().unwrap());
    assert_eq!(empty.trim(), "[]");

    ok(ssh(&home)
        .args([
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
        ])
        .output()
        .unwrap());
    ok(ssh(&home)
        .args([
            "add",
            "keyed",
            "--host",
            "10.0.0.9",
            "--username",
            "ubuntu",
            "--key",
            "/tmp/id_ed25519",
            "--passphrase",
            "",
        ])
        .output()
        .unwrap());

    let output = ok(ssh(&home).args(["list", "--json"]).output().unwrap());
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let entries = parsed.as_array().unwrap();
    assert_eq!(entries.len(), 2);

    let keyed = &entries[0];
    assert_eq!(keyed["name"], "keyed");
    assert_eq!(keyed["host"], "10.0.0.9");
    assert_eq!(keyed["port"], 22);
    assert_eq!(keyed["username"], "ubuntu");
    assert_eq!(keyed["auth_type"], "key");
    assert_eq!(keyed["key_path"], "/tmp/id_ed25519");

    let prod = &entries[1];
    assert_eq!(prod["name"], "prod");
    assert_eq!(prod["port"], 2222);
    assert_eq!(prod["auth_type"], "password");
    assert!(prod["key_path"].is_null());
    assert!(!output.contains("p@ssw0rd"), "{output}");
}

#[test]
fn add_reports_error_for_malformed_store() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let data_dir = home.join("data/ssh");
    fs::create_dir_all(&data_dir).unwrap();
    let database = data_dir.join("servers.sqlite3");
    let connection = rusqlite::Connection::open(&database).unwrap();
    connection
        .execute_batch("CREATE TABLE servers (name TEXT PRIMARY KEY)")
        .unwrap();
    drop(connection);

    let output = ssh(&home)
        .args([
            "add",
            "bad",
            "--host",
            "10.0.0.1",
            "--username",
            "root",
            "--password",
            "p@ssw0rd",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("dm ssh:"));
}
