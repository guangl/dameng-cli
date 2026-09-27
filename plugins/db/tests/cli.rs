//! End-to-end tests: the plugin binary runs under the host protocol environment.
//! The driver behind test/exec is still a placeholder, so these tests only cover
//! the paths that fail before or at the driver, never a real connection.

use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Output, Stdio},
};
use tempfile::TempDir;

fn db(home: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_dm-db"));
    command
        .env("DM_PLUGIN_API_VERSION", "1")
        .env("DM_PLUGIN_CAPABILITIES", "config-dirs-v1")
        .env("DM_PLUGIN_DIR", home.join("plugins/db"))
        .env("DM_PLUGIN_HOME", home)
        .env("DM_PLUGIN_CONFIG_DIR", home.join("config/db"))
        .env("DM_PLUGIN_DATA_DIR", home.join("data/db"))
        .env("DM_PLUGIN_CACHE_DIR", home.join("cache/db"));
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

fn failure(output: Output) -> String {
    assert!(!output.status.success(), "expected a failing command");
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn write_plugin_config(home: &Path, text: &str) {
    let directory = home.join("config/db");
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join("config.toml"), text).unwrap();
}

#[test]
fn add_list_and_remove_connections() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();

    let add = ok(db(&home)
        .args([
            "add",
            "prod",
            "--host",
            "10.0.0.8",
            "--port",
            "5237",
            "--username",
            "SYSDBA",
            "--password",
            "SYSDBA@123",
            "--schema",
            "DMHR",
        ])
        .output()
        .unwrap());
    assert!(add.contains("Saved database connection prod"), "{add}");
    assert!(
        !add.contains("SYSDBA@123"),
        "the password must not be echoed"
    );

    let list = ok(db(&home).args(["list"]).output().unwrap());
    assert!(list.contains("prod\tSYSDBA@10.0.0.8:5237\tDMHR"), "{list}");
    assert!(list.contains("DM8 ODBC DRIVER"), "{list}");
    assert!(!list.contains("SYSDBA@123"), "{list}");

    let remove = ok(db(&home).args(["remove", "prod"]).output().unwrap());
    assert!(
        remove.contains("Removed database connection prod"),
        "{remove}"
    );
    assert!(ok(db(&home).args(["list"]).output().unwrap()).is_empty());
}

#[test]
fn add_uses_the_plugin_configuration_defaults() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    write_plugin_config(
        &home,
        concat!(
            "[defaults]\nport = 5300\nusername = \"DMHR\"\n",
            "driver = \"DM8 ODBC DRIVER\"\nschema = \"DMHR\"\n",
        ),
    );

    ok(db(&home)
        .args(["add", "prod", "--host", "10.0.0.8", "--password", "pw"])
        .output()
        .unwrap());
    let list = ok(db(&home).args(["list"]).output().unwrap());
    assert!(list.contains("DMHR@10.0.0.8:5300\tDMHR"), "{list}");

    // An explicit flag wins over the plugin configuration.
    ok(db(&home)
        .args([
            "add",
            "explicit",
            "--host",
            "10.0.0.9",
            "--port",
            "5236",
            "--username",
            "SYSDBA",
            "--password",
            "pw",
            "--schema",
            "",
        ])
        .output()
        .unwrap());
    let list = ok(db(&home).args(["list"]).output().unwrap());
    assert!(list.contains("SYSDBA@10.0.0.9:5236\t-"), "{list}");
}

#[test]
fn invalid_plugin_configuration_is_reported() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    write_plugin_config(&home, "[connect]\ntimeout = 0\n");

    // Commands that do not need the values still work.
    assert!(db(&home).args(["list"]).output().unwrap().status.success());

    let stderr = failure(
        db(&home)
            .args(["add", "prod", "--host", "10.0.0.8", "--password", "pw"])
            .output()
            .unwrap(),
    );
    assert!(stderr.contains("connect.timeout"), "{stderr}");
    assert!(stderr.contains("config.toml"), "{stderr}");
}

#[test]
fn malformed_store_is_reported() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let data_dir = home.join("data/db");
    fs::create_dir_all(&data_dir).unwrap();
    let database = data_dir.join("connections.sqlite3");
    let connection = rusqlite::Connection::open(&database).unwrap();
    connection
        .execute_batch("CREATE TABLE connections (name TEXT PRIMARY KEY)")
        .unwrap();
    drop(connection);

    let stderr = failure(
        db(&home)
            .args(["add", "prod", "--host", "10.0.0.8", "--password", "pw"])
            .output()
            .unwrap(),
    );
    assert!(stderr.contains("dm db:"), "{stderr}");
    assert!(stderr.contains("connections.sqlite3"), "{stderr}");
}

#[test]
fn missing_arguments_and_connections_fail_with_hints() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();

    // Without a terminal the plugin refuses to guess the password.
    let stderr = failure(
        db(&home)
            .args(["add", "prod", "--host", "10.0.0.8"])
            .output()
            .unwrap(),
    );
    assert!(stderr.contains("--password"), "{stderr}");
    assert!(stderr.contains("提示"), "{stderr}");

    assert!(ok(db(&home).args(["list"]).output().unwrap()).is_empty());

    let stderr = failure(db(&home).args(["test", "missing"]).output().unwrap());
    assert!(stderr.contains("not configured"), "{stderr}");
    assert!(stderr.contains("dm db add"), "{stderr}");

    let stderr = failure(db(&home).args(["remove", "missing"]).output().unwrap());
    assert!(stderr.contains("not configured"), "{stderr}");
}

#[test]
fn test_reports_the_deferred_driver() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    ok(db(&home)
        .args(["add", "prod", "--host", "10.0.0.8", "--password", "pw"])
        .output()
        .unwrap());

    let stderr = failure(db(&home).args(["test", "prod"]).output().unwrap());
    assert!(stderr.contains("driver is not implemented"), "{stderr}");
    assert!(stderr.contains("尚未接入"), "{stderr}");
}

#[test]
fn exec_reads_sql_from_stdin_or_a_file() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    ok(db(&home)
        .args(["add", "prod", "--host", "10.0.0.8", "--password", "pw"])
        .output()
        .unwrap());

    // stdin is read before the driver is opened, so the statement is accepted
    // and only then is the deferred driver reported.
    let mut child = db(&home)
        .args(["exec", "prod"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"SELECT 1 FROM DUAL;")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    let stderr = failure(output);
    assert!(stderr.contains("driver is not implemented"), "{stderr}");

    // Empty stdin is rejected without touching the store.
    let mut child = db(&home)
        .args(["exec", "prod"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"   \n").unwrap();
    let stderr = failure(child.wait_with_output().unwrap());
    assert!(stderr.contains("SQL must not be empty"), "{stderr}");

    // A missing SQL file is reported as a file error.
    let missing = temp.path().join("missing.sql");
    let stderr = failure(
        db(&home)
            .args(["exec", "prod", "--file", missing.to_str().unwrap()])
            .output()
            .unwrap(),
    );
    assert!(stderr.contains("missing.sql"), "{stderr}");

    // Unknown connections are reported before any driver is loaded.
    let stderr = failure(
        db(&home)
            .args(["exec", "missing", "SELECT 1"])
            .output()
            .unwrap(),
    );
    assert!(stderr.contains("not configured"), "{stderr}");
}

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
