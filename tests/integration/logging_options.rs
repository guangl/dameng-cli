use crate::common::*;
use dameng_cli::logging::log_file_path;
use std::fs;
use tempfile::TempDir;

#[test]
fn custom_directory_and_environment_overrides_stay_off_the_terminal() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    fs::create_dir(&home).unwrap();
    write_config(&home, "[log]\ndirectory = 'diagnostics'\nmax_size_mb = 1\n");
    let destination = home.join("diagnostics");
    let file_name = log_file_path(&home).file_name().unwrap().to_owned();
    fs::create_dir(&destination).unwrap();
    fs::write(destination.join(&file_name), vec![b'x'; 2 * 1024 * 1024]).unwrap();
    let output = dm_isolated(&home).args(["doctor"]).output().unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert!(!String::from_utf8_lossy(&output.stdout).contains("running doctor"));
    assert!(destination.join(&file_name).metadata().unwrap().len() <= 1024 * 1024);
    assert!(!log_file_path(&home).exists());
    let other = temp.path().join("absolute logs");
    ok(dm_isolated(&home)
        .env("DM_LOG_DIR", &other)
        .env("DM_LOG_MAX_SIZE_MB", "2")
        .args(["doctor"])
        .output()
        .unwrap());
    assert!(
        fs::read_to_string(other.join(&file_name))
            .unwrap()
            .contains("running doctor")
    );
    let show = ok(dm_isolated(&home)
        .env("DM_LOG_DIR", &other)
        .env("DM_LOG_MAX_SIZE_MB", "2")
        .args(["config", "show", "--json"])
        .output()
        .unwrap());
    let parsed: serde_json::Value = serde_json::from_str(&show).unwrap();
    for (key, source) in [
        ("log.directory", "env:DM_LOG_DIR"),
        ("log.max_size_mb", "env:DM_LOG_MAX_SIZE_MB"),
    ] {
        assert!(
            parsed["settings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|entry| entry["key"] == key && entry["source"] == source)
        );
    }
}

#[test]
fn logging_failures_and_invalid_filters_never_print_diagnostics() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    fs::create_dir(&home).unwrap();
    let blocked = temp.path().join("blocked");
    fs::write(&blocked, "not a directory").unwrap();
    for filter in ["info", "not-a-level"] {
        let output = dm_isolated(&home)
            .env("DM_LOG_DIR", &blocked)
            .env("DM_LOG", filter)
            .args(["doctor"])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(output.stderr.is_empty(), "{}", stderr(&output));
        assert!(!String::from_utf8_lossy(&output.stdout).contains("running doctor"));
    }
    for value in ["0", "-1", "18446744073709551615", "invalid"] {
        let output = dm_isolated(&home)
            .env("DM_LOG_MAX_SIZE_MB", value)
            .args(["list"])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(!stderr(&output).contains("[ERROR]"));
    }
}
