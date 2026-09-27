//! Diagnostics belong in the host log file, not on the terminal.

use crate::common::*;
use std::fs;
use tempfile::TempDir;

#[test]
fn diagnostics_go_to_the_log_file_instead_of_stderr() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");

    let output = dm_isolated(&home).args(["doctor"]).output().unwrap();
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("running doctor"), "{stderr}");

    let log = fs::read_to_string(home.join("dm.log")).unwrap();
    assert!(log.contains("running doctor repair=false"), "{log}");
}

#[test]
fn failures_keep_the_report_on_stderr_and_the_error_in_the_log() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");

    let output = dm_isolated(&home)
        .args(["info", "missing"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("错误："), "{stderr}");
    assert!(!stderr.contains("[ERROR]"), "{stderr}");

    let log = fs::read_to_string(home.join("dm.log")).unwrap();
    assert!(log.contains("ERROR"), "{log}");
    assert!(log.contains("is not installed"), "{log}");
}

#[test]
fn turning_logging_off_writes_no_log_file() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");

    let output = dm_isolated(&home)
        .env("DM_LOG", "off")
        .args(["doctor"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(!home.join("dm.log").exists());
}

#[test]
fn a_broken_configuration_still_honours_dm_log_off() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    write_config(&home, "not = valid = toml\n");

    let output = dm_isolated(&home)
        .env("DM_LOG", "off")
        .args(["list"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!home.join("dm.log").exists());
}

#[test]
fn a_broken_configuration_is_logged_without_dm_log_off() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    write_config(&home, "not = valid = toml\n");

    let output = dm_isolated(&home).args(["list"]).output().unwrap();
    assert!(!output.status.success());
    let log = fs::read_to_string(home.join("dm.log")).unwrap();
    assert!(log.contains("Invalid configuration"), "{log}");
}

#[test]
fn an_unusable_data_directory_falls_back_to_stderr() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    fs::write(&home, b"not a directory").unwrap();

    // A filter stricter than the notice must not swallow it.
    let output = dm_isolated(&home)
        .env("DM_LOG", "error")
        .args(["list"])
        .output()
        .unwrap();
    let stderr = stderr(&output);
    assert!(stderr.contains("cannot write the log file"), "{stderr}");
}
