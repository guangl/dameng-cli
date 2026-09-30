//! End-to-end install, run and uninstall of a locally built plugin.

use crate::common::*;
use std::fs;
use std::io::Write;
use std::process::Stdio;
use tempfile::TempDir;

#[test]
fn rust_plugin_lifecycle_and_process_contract() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("dm home");
    let source = fixture(temp.path());
    assert!(ok(dm(&home).arg("list").output().unwrap()).contains("No plugins installed"));
    assert!(home.join("store.sqlite3").is_file());
    assert!(
        ok(dm(&home).arg("install").arg(&source).output().unwrap()).contains("Installed probe")
    );
    let listed = ok(dm(&home).arg("list").output().unwrap());
    assert!(listed.contains("probe") && listed.contains("0.1.0"));
    assert_eq!(
        &fs::read(home.join("store.sqlite3")).unwrap()[..16],
        b"SQLite format 3\0"
    );
    assert!(
        !dm(&home)
            .arg("install")
            .arg(&source)
            .output()
            .unwrap()
            .status
            .success()
    );
    // Runtime must be independent of the source tree and build products.
    fs::remove_dir_all(&source).unwrap();
    let mut child = dm(&home)
        .args(["probe", "hello world", "--help", "--", "--fail"])
        .env("DM_TEST_SECRET", "must-not-leak")
        .current_dir(temp.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"input payload\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(23));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("arg=hello world\narg=--help\narg=--\narg=--fail\n"),
        "{stdout}"
    );
    assert!(stdout.contains("stdin=input payload"));
    let cwd = stdout
        .lines()
        .find_map(|line| line.strip_prefix("cwd="))
        .unwrap();
    assert_eq!(
        fs::canonicalize(cwd).unwrap(),
        fs::canonicalize(temp.path()).unwrap()
    );
    assert!(stdout.contains(&format!(
        "home={}",
        fs::canonicalize(&home).unwrap().display()
    )));
    assert!(stdout.contains(&format!(
        "legacy_home={}",
        fs::canonicalize(&home).unwrap().display()
    )));
    assert!(stdout.contains(
        &format!("plugin={}", fs::canonicalize(home.join("plugins/probe")).unwrap().display())
    ));
    for directory in ["config", "data", "cache"] {
        assert!(stdout.contains(&format!(
            "{directory}={}",
            fs::canonicalize(home.join(directory).join("probe"))
                .unwrap()
                .display()
        )));
    }
    assert!(stdout.contains("capabilities=config-dirs-v1"));
    assert!(stdout.contains("secret=filtered"));
    assert!(String::from_utf8_lossy(&output.stderr).contains("plugin stderr"));
    // A broken plugin must remain removable.
    fs::write(home.join("plugins/probe/dm-plugin.toml"), "broken").unwrap();
    ok(dm(&home).args(["uninstall", "probe"]).output().unwrap());
    assert!(ok(dm(&home).arg("list").output().unwrap()).contains("No plugins installed"));
    for directory in ["config", "data", "cache"] {
        assert!(!home.join(directory).join("probe").exists());
    }
    assert!(!dm(&home).arg("probe").output().unwrap().status.success());
}
#[test]
fn install_replace_upgrades_in_place_and_keeps_plugin_data() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    ok(dm(&home)
        .args(["install", source.to_str().unwrap()])
        .output()
        .unwrap());

    // Data the plugin wrote itself has to survive a replacement.
    let data = home.join("data/probe");
    fs::create_dir_all(&data).unwrap();
    fs::write(data.join("state.txt"), "kept").unwrap();

    // Without --replace an installed plugin is still refused, and the error
    // points at both ways forward.
    let refused = dm(&home)
        .args(["install", source.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!refused.status.success());
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert!(stderr.contains("already installed"), "{stderr}");
    assert!(stderr.contains("--replace"), "{stderr}");

    // A newer package of the same plugin replaces the installed one.
    let newer = temp.path().join("probe 0.2.0");
    fs::create_dir_all(&newer).unwrap();
    fs::write(
        newer.join("dm-plugin.toml"),
        manifest("probe").replace("0.1.0", "0.2.0"),
    )
    .unwrap();
    fs::copy(
        source.join(format!("dm-probe{}", std::env::consts::EXE_SUFFIX)),
        newer.join(format!("dm-probe{}", std::env::consts::EXE_SUFFIX)),
    )
    .unwrap();
    let replaced = ok(dm(&home)
        .args(["install", newer.to_str().unwrap(), "--replace"])
        .output()
        .unwrap());
    assert!(replaced.contains("Installed probe 0.2.0"), "{replaced}");
    assert!(ok(dm(&home).arg("list").output().unwrap()).contains("0.2.0"));
    assert_eq!(fs::read_to_string(data.join("state.txt")).unwrap(), "kept");
    assert_eq!(
        dameng_cli::PluginStore::new(&home)
            .verify(Some("probe"))
            .unwrap(),
        ["probe"]
    );

    // Replacing also covers the first installation of a plugin.
    ok(dm(&home).args(["uninstall", "probe"]).output().unwrap());
    ok(dm(&home)
        .args(["install", newer.to_str().unwrap(), "--replace"])
        .output()
        .unwrap());
    assert!(ok(dm(&home).arg("list").output().unwrap()).contains("probe"));
}
