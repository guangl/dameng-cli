use std::{fs, path::Path, process::Command};
use tempfile::TempDir;

/// Build a `dm` invocation that is isolated from the developer's own host state.
fn dm(home: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_dm"));
    command
        .env("DM_PLUGIN_HOME", home)
        .env_remove("DM_LOG")
        .env_remove("RUST_LOG")
        .env_remove("DM_UPDATE_REPOSITORY");
    command
}

fn write_config(home: &Path, text: &str) {
    fs::write(home.join("config.toml"), text).unwrap();
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn config_file_sets_log_filter() {
    let temp = TempDir::new().unwrap();
    write_config(temp.path(), "log = \"debug\"\n");

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
fn environment_overrides_repository_from_config_file() {
    let temp = TempDir::new().unwrap();
    write_config(temp.path(), "update_repository = \"config-only\"\n");

    let args = ["self-update", "--check", "--version", "0.0.1", "--json"];
    let output = dm(temp.path()).args(args).output().unwrap();
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("config-only"),
        "stderr: {}",
        stderr(&output)
    );

    let output = dm(temp.path())
        .env("DM_UPDATE_REPOSITORY", "example/override")
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", stderr(&output));
    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(parsed["updated"], false);
    assert_eq!(parsed["available_version"], "0.0.1");
}

#[test]
fn invalid_config_file_reports_actionable_error() {
    let temp = TempDir::new().unwrap();
    write_config(temp.path(), "log = \"debug\"\nunknown_key = 1\n");

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
    write_config(temp.path(), "log = \"  \"\n");

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
    assert_eq!(parsed.log.as_deref(), Some("info"));
    assert_eq!(
        parsed.update_repository.as_deref(),
        Some("guangl/dameng-cli")
    );

    let temp = TempDir::new().unwrap();
    write_config(temp.path(), &text);
    let output = dm(temp.path()).arg("list").output().unwrap();
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(String::from_utf8_lossy(&output.stdout).contains("No plugins installed"));
}
