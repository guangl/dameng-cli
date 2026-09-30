use std::fs;

use tempfile::TempDir;

use crate::common::*;

#[test]
fn plugin_config_supplies_add_defaults() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    write_plugin_config(&home, "[defaults]\nport = 2200\nusername = \"ubuntu\"\n");

    let add = ok(ssh(&home)
        .args([
            "add",
            "prod",
            "--host",
            "10.0.0.8",
            "--password",
            "p@ssw0rd",
        ])
        .output()
        .unwrap());
    assert!(add.contains("已保存 SSH 连接 prod"), "{add}");

    let list = ok(ssh(&home).args(["list"]).output().unwrap());
    for needle in ["ubuntu", "10.0.0.8", "2200"] {
        assert!(list.contains(needle), "missing {needle:?} in:\n{list}");
    }

    // An explicit flag still wins over the plugin configuration.
    ok(ssh(&home)
        .args([
            "add",
            "explicit",
            "--host",
            "10.0.0.9",
            "--port",
            "2222",
            "--username",
            "root",
            "--password",
            "p@ssw0rd",
        ])
        .output()
        .unwrap());
    let list = ok(ssh(&home).args(["list"]).output().unwrap());
    for needle in ["root", "10.0.0.9", "2222"] {
        assert!(list.contains(needle), "missing {needle:?} in:\n{list}");
    }
}

#[test]
fn invalid_plugin_config_is_reported() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    write_plugin_config(&home, "[defaults]\nauth = \"token\"\n");

    // Commands that do not need the values still work.
    assert!(ssh(&home).args(["list"]).output().unwrap().status.success());

    let output = ssh(&home)
        .args([
            "add",
            "prod",
            "--host",
            "10.0.0.8",
            "--username",
            "root",
            "--password",
            "pw",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("defaults.auth"), "{stderr}");
    assert!(stderr.contains("config.toml"), "{stderr}");
}

#[cfg(unix)]
#[test]
fn plugin_config_sets_the_test_connect_timeout() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    write_plugin_config(&home, "[test]\nconnect_timeout = 3\n");

    let tools = temp.path().join("tools");
    fs::create_dir(&tools).unwrap();
    let log = temp.path().join("ssh-args.log");
    let ssh_script = tools.join("ssh");
    fs::write(
        &ssh_script,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$FAKE_SSH_LOG\"\nexit 0\n",
    )
    .unwrap();
    fs::set_permissions(&ssh_script, fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::join_paths(
        std::iter::once(tools).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();

    ok(ssh(&home)
        .args([
            "add",
            "prod",
            "--host",
            "10.0.0.8",
            "--username",
            "root",
            "--key",
            "/tmp/id_ed25519",
        ])
        .output()
        .unwrap());

    let mut test = ssh(&home);
    test.env("PATH", &path).env("FAKE_SSH_LOG", &log);
    ok(test.args(["test", "prod"]).output().unwrap());

    let args = fs::read_to_string(&log).unwrap();
    assert!(args.contains("ConnectTimeout=3"), "{args}");
}

#[test]
fn plugin_config_supplies_a_default_key() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    write_plugin_config(
        &home,
        concat!(
            "[defaults]\nusername = \"ubuntu\"\n",
            "auth = \"key\"\nkey = \"/tmp/id_ed25519\"\n",
        ),
    );

    ok(ssh(&home)
        .args(["add", "keyed", "--host", "10.0.0.9"])
        .output()
        .unwrap());

    let list = ok(ssh(&home).args(["list"]).output().unwrap());
    for needle in ["ubuntu", "10.0.0.9", "key", "/tmp/id_ed25519"] {
        assert!(list.contains(needle), "missing {needle:?} in:\n{list}");
    }
}
