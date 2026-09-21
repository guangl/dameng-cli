use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
use tempfile::TempDir;

fn ssh(home: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_dm-ssh"));
    command
        .env("DM_PLUGIN_API_VERSION", "1")
        .env("DM_PLUGIN_CAPABILITIES", "config-dirs-v1")
        .env("DM_PLUGIN_DIR", home.join("plugins/ssh"))
        .env("DM_PLUGIN_HOME", home)
        .env("DM_PLUGIN_CONFIG_DIR", home.join("config/ssh"))
        .env("DM_PLUGIN_DATA_DIR", home.join("data/ssh"))
        .env("DM_PLUGIN_CACHE_DIR", home.join("cache/ssh"));
    command
}

fn ok(output: Output) -> String {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

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
    assert!(add.contains("Saved SSH server prod"), "{add}");

    let list = ok(ssh(&home).args(["list"]).output().unwrap());
    assert!(list.contains("prod"), "{list}");
    assert!(list.contains("10.0.0.8"), "{list}");
    assert!(list.contains("2222"), "{list}");

    let remove = ok(ssh(&home).args(["remove", "prod"]).output().unwrap());
    assert!(remove.contains("Removed SSH server prod"), "{remove}");
    assert!(ok(ssh(&home).args(["list"]).output().unwrap()).is_empty());
}

#[cfg(unix)]
#[test]
fn add_with_key_reports_and_key_auth_runs_ssh() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();

    let tools = temp.path().join("tools");
    fs::create_dir(&tools).unwrap();
    let ssh_script = tools.join("ssh");
    fs::write(&ssh_script, "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&ssh_script, fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::join_paths(
        std::iter::once(tools).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();

    let add = ok(ssh(&home)
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
            "secret",
        ])
        .output()
        .unwrap());
    assert!(add.contains("Saved SSH server keyed"), "{add}");

    let list = ok(ssh(&home).args(["list"]).output().unwrap());
    assert!(list.contains("key"), "{list}");

    let mut test = ssh(&home);
    test.env("PATH", &path);
    let test_out = ok(test.args(["test", "keyed"]).output().unwrap());
    assert!(
        test_out.contains("SSH server keyed is reachable"),
        "{test_out}"
    );
}

#[cfg(unix)]
#[test]
fn password_auth_test_and_ssh_exit_codes_are_preserved() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();

    let tools = temp.path().join("tools");
    fs::create_dir(&tools).unwrap();
    let sshpass = tools.join("sshpass");
    fs::write(&sshpass, "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&sshpass, fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::join_paths(
        std::iter::once(tools).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();

    ok(ssh(&home)
        .args([
            "add",
            "pw",
            "--host",
            "10.0.0.10",
            "--username",
            "root",
            "--password",
            "p@ssw0rd",
        ])
        .output()
        .unwrap());

    let mut test = ssh(&home);
    test.env("PATH", &path);
    let test_out = ok(test.args(["test", "pw"]).output().unwrap());
    assert!(
        test_out.contains("SSH server pw is reachable"),
        "{test_out}"
    );

    fs::write(&sshpass, "#!/bin/sh\nexit 7\n").unwrap();
    let mut session = ssh(&home);
    session.env("PATH", &path);
    let output = session.args(["ssh", "pw"]).output().unwrap();
    assert_eq!(output.status.code(), Some(7));

    fs::write(&sshpass, "#!/bin/sh\nkill -TERM $$\n").unwrap();
    let mut signalled = ssh(&home);
    signalled.env("PATH", &path);
    let output = signalled.args(["ssh", "pw"]).output().unwrap();
    assert_eq!(output.status.code(), Some(143));
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
