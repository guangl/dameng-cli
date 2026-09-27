//! The optional `config.toml` file: parsing, validation and errors.

use crate::common::dm_isolated as dm;
use crate::common::*;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn config_file_sets_log_filter() {
    let temp = TempDir::new().unwrap();
    write_config(temp.path(), "[log]\nlevel = \"debug\"\n");

    let output = dm(temp.path()).arg("list").output().unwrap();
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("opened plugin store"),
        "stderr: {}",
        stderr(&output)
    );

    // Without the file the documented default of `info` keeps debug lines hidden.
    let default_home = TempDir::new().unwrap();
    let output = dm(default_home.path()).arg("list").output().unwrap();
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(!stderr(&output).contains("opened plugin store"));
}
#[test]
fn invalid_config_file_reports_actionable_error() {
    let temp = TempDir::new().unwrap();
    write_config(temp.path(), "[log]\nlevel = \"debug\"\nunknown_key = 1\n");

    let output = dm(temp.path()).arg("list").output().unwrap();
    assert!(!output.status.success());
    let stderr = stderr(&output);
    assert!(stderr.contains("config.toml"), "stderr: {stderr}");
    assert!(stderr.contains("unknown field"), "stderr: {stderr}");
    assert!(
        stderr.contains("错误：") && stderr.contains("提示："),
        "stderr: {stderr}"
    );
}
#[test]
fn empty_config_value_is_rejected() {
    let temp = TempDir::new().unwrap();
    write_config(temp.path(), "[log]\nlevel = \"  \"\n");

    let output = dm(temp.path()).arg("list").output().unwrap();
    assert!(!output.status.success());
    let stderr = stderr(&output);
    assert!(stderr.contains("config.toml"), "stderr: {stderr}");
    assert!(stderr.contains("must not be empty"), "stderr: {stderr}");
}
#[test]
fn unreadable_config_path_reports_the_file() {
    let temp = TempDir::new().unwrap();
    // A directory where the file is expected fails on every platform, without
    // depending on the process losing read permission.
    fs::create_dir(temp.path().join("config.toml")).unwrap();

    let output = dm(temp.path()).arg("list").output().unwrap();
    assert!(!output.status.success());
    let stderr = stderr(&output);
    assert!(stderr.contains("config.toml"), "stderr: {stderr}");
    assert!(stderr.contains("提示："), "stderr: {stderr}");
}
#[test]
fn unresolvable_data_directory_is_reported() {
    let output = Command::new(env!("CARGO_BIN_EXE_dm"))
        .env_remove("DM_PLUGIN_HOME")
        .env_remove("HOME")
        .env_remove("LOCALAPPDATA")
        .arg("list")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = stderr(&output);
    assert!(
        stderr.contains("DM_PLUGIN_HOME or HOME")
            || stderr.contains("DM_PLUGIN_HOME or LOCALAPPDATA"),
        "stderr: {stderr}"
    );
}
#[test]
fn shipped_example_config_is_accepted_by_the_host() {
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/config.toml");
    let text = fs::read_to_string(&example).unwrap();

    // `deny_unknown_fields` means this also proves the demo only uses supported keys.
    let parsed = dameng_cli::Config::from_toml(&text).unwrap();
    assert_eq!(parsed.log.level.as_deref(), Some("info"));
    assert_eq!(
        parsed.update.repository.as_deref(),
        Some("guangl/dameng-cli")
    );

    // Optional keys stay commented out, so copying the example cannot change behavior.
    assert_eq!(parsed.update.target, None);
    assert_eq!(parsed.output.progress, None);
    assert!(parsed.plugin.environment.is_empty());

    let temp = TempDir::new().unwrap();
    write_config(temp.path(), &text);
    let output = dm(temp.path()).arg("list").output().unwrap();
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(String::from_utf8_lossy(&output.stdout).contains("No plugins installed"));
}
#[test]
fn invalid_plugin_environment_entry_names_the_key() {
    let temp = TempDir::new().unwrap();
    write_config(temp.path(), "[plugin]\nenvironment = [\"DB-URL\"]\n");

    let output = dm(temp.path()).arg("list").output().unwrap();
    assert!(!output.status.success());
    let stderr = stderr(&output);
    assert!(stderr.contains("plugin.environment"), "stderr: {stderr}");
    assert!(stderr.contains("提示："), "stderr: {stderr}");
}
#[test]
fn invalid_progress_override_in_the_environment_is_rejected() {
    let temp = TempDir::new().unwrap();

    let output = dm(temp.path())
        .env("DM_PROGRESS", "maybe")
        .arg("list")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = stderr(&output);
    assert!(
        stderr.contains("DM_PROGRESS must be true or false"),
        "stderr: {stderr}"
    );
}
#[test]
fn progress_switch_is_accepted_from_the_file_and_the_environment() {
    let temp = TempDir::new().unwrap();
    write_config(temp.path(), "[output]\nprogress = false\n");
    assert!(
        dm(temp.path())
            .arg("list")
            .output()
            .unwrap()
            .status
            .success()
    );

    for value in ["true", "0", "off", "yes"] {
        let output = dm(temp.path())
            .env("DM_PROGRESS", value)
            .arg("list")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "DM_PROGRESS={value}: {}",
            stderr(&output)
        );
    }
}
