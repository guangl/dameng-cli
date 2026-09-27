//! Installing from a Git source, plus concurrent installs of one package.

use crate::common::*;
use std::fs;
use std::process::Command;
use std::process::Stdio;
use tempfile::TempDir;

#[test]
fn concurrent_install_publishes_one_complete_plugin() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    let first = dm(&home)
        .arg("install")
        .arg(&source)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let second = dm(&home)
        .arg("install")
        .arg(&source)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let results = [
        first.wait_with_output().unwrap(),
        second.wait_with_output().unwrap(),
    ];
    assert_eq!(results.iter().filter(|r| r.status.success()).count(), 1);
    let listed = ok(dm(&home).arg("list").output().unwrap());
    assert!(listed.contains("probe") && listed.contains("0.1.0"));
    assert_eq!(fs::read_dir(home.join("plugins")).unwrap().count(), 1);
    let binary = home
        .join("plugins/probe")
        .join(format!("dm-probe{}", std::env::consts::EXE_SUFFIX));
    let output = Command::new(&binary)
        .env_remove("DM_PLUGIN_API_VERSION")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Run this plugin through dm"));
    let output = Command::new(binary)
        .env("DM_PLUGIN_API_VERSION", "999")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Unsupported host plugin API"));
}
#[cfg(unix)]
#[test]
fn checkout_revision_failure_is_reported() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    let tools = temp.path().join("tools");
    fs::create_dir(&tools).unwrap();

    let git = tools.join("git");
    fs::write(
        &git,
        r#"#!/bin/sh
last=""
for arg do last="$arg"; done
case " $* " in
  *" checkout "*) exit 1;;
  *" clone "*) cp -R "$FAKE_GIT_SOURCE" "$last";;
  *" rev-parse "*) printf '%040d
' 1;;
esac
"#,
    )
    .unwrap();
    fs::set_permissions(&git, fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::join_paths(
        std::iter::once(tools).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();

    let output = dm(&home)
        .env("PATH", &path)
        .env("FAKE_GIT_SOURCE", &source)
        .args([
            "install",
            "https://example.invalid/probe.git",
            "--rev",
            "bad",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("could not be checked out"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
#[cfg(unix)]
#[test]
fn clone_failure_reports_the_git_error() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let tools = temp.path().join("tools");
    fs::create_dir(&tools).unwrap();

    let git = tools.join("git");
    fs::write(
        &git,
        "#!/bin/sh\ncase \" $* \" in\n  *\" clone \"*) echo 'fatal: repository not found' >&2; exit 128;;\nesac\nexit 1\n",
    )
    .unwrap();
    fs::set_permissions(&git, fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::join_paths(
        std::iter::once(tools).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();

    let output = dm(&home)
        .env("PATH", &path)
        .args(["install", "https://example.invalid/probe.git"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Git could not fetch the plugin"),
        "{stderr}"
    );
    assert!(stderr.contains("repository not found"), "{stderr}");
}
#[test]
fn checkout_rejects_whitespace_revision() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let output = dm(&home)
        .args([
            "install",
            "https://example.invalid/probe.git",
            "--rev",
            "bad rev",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("revision"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
