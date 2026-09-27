use std::fs;

use tempfile::TempDir;

use crate::common::*;

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
