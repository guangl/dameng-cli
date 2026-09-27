//! End-to-end tests for the command line help and the host protocol guard.

use crate::common::*;
use std::{fs, process::Command};
use tempfile::TempDir;

#[test]
fn help_lists_every_subcommand() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();

    let help = ok(db(&home).arg("--help").output().unwrap());
    for command in ["add", "list", "remove", "test", "exec"] {
        assert!(help.contains(command), "{help}");
    }
    assert!(help.contains("config.toml"), "{help}");

    // The plugin reports the host protocol when it is run without dm.
    let direct = Command::new(env!("CARGO_BIN_EXE_dm-db"))
        .arg("list")
        .output()
        .unwrap();
    assert!(!direct.status.success());
    assert!(
        String::from_utf8_lossy(&direct.stderr).contains("dm <plugin>"),
        "{}",
        String::from_utf8_lossy(&direct.stderr)
    );
}
