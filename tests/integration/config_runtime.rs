//! Config settings that change runtime behavior: update and plugin environment.

use crate::common::dm_isolated as dm;
use crate::common::*;
use std::fs;
use tempfile::TempDir;

#[test]
fn environment_overrides_repository_from_config_file() {
    let temp = TempDir::new().unwrap();
    write_config(temp.path(), "[update]\nrepository = \"config-only\"\n");

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
fn configured_update_target_is_validated_before_any_download() {
    let temp = TempDir::new().unwrap();
    write_config(
        temp.path(),
        "[update]\ntarget = \"mips-unknown-linux-gnu\"\n",
    );

    // `--force` skips the version comparison so the target check runs; it happens
    // before the first download, so this stays offline.
    let args = ["self-update", "--version", "0.0.1", "--force"];
    let output = dm(temp.path()).args(args).output().unwrap();
    assert!(!output.status.success());
    let rejected = stderr(&output);
    assert!(
        rejected.contains("not published for target mips-unknown-linux-gnu"),
        "stderr: {rejected}"
    );
    assert!(
        rejected.contains("aarch64-apple-darwin"),
        "stderr: {rejected}"
    );

    // DM_UPDATE_TARGET wins over the file value and is validated the same way.
    write_config(temp.path(), "[update]\ntarget = \"aarch64-apple-darwin\"\n");
    let output = dm(temp.path())
        .env("DM_UPDATE_TARGET", "sparc-unknown-linux-gnu")
        .args(args)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("sparc-unknown-linux-gnu"),
        "{}",
        stderr(&output)
    );
}
#[test]
fn configured_plugin_environment_is_inherited_by_plugins() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    ok(dm(&home).arg("install").arg(&source).output().unwrap());

    // The manifest does not declare DM_TEST_SECRET, so the host filters it out.
    let filtered = ok(dm(&home)
        .env("DM_TEST_SECRET", "must-not-leak")
        .arg("probe")
        .output()
        .unwrap());
    assert!(filtered.contains("secret=filtered"), "{filtered}");

    // config.toml can grant it globally instead of per manifest.
    fs::write(
        home.join("config.toml"),
        "[plugin]\nenvironment = [\"DM_TEST_SECRET\"]\n",
    )
    .unwrap();
    let inherited = ok(dm(&home)
        .env("DM_TEST_SECRET", "granted-by-config")
        .arg("probe")
        .output()
        .unwrap());
    assert!(
        inherited.contains("secret=granted-by-config"),
        "{inherited}"
    );

    // DM_PLUGIN_ENVIRONMENT replaces the file list instead of extending it.
    let replaced = ok(dm(&home)
        .env("DM_TEST_SECRET", "must-not-leak")
        .env("DM_PLUGIN_ENVIRONMENT", "DM_OTHER_VALUE")
        .arg("probe")
        .output()
        .unwrap());
    assert!(replaced.contains("secret=filtered"), "{replaced}");

    // Invalid names fail loudly instead of silently inheriting nothing.
    let output = dm(&home)
        .env("DM_PLUGIN_ENVIRONMENT", "NOT-A-NAME")
        .arg("probe")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("DM_PLUGIN_ENVIRONMENT"), "{stderr}");
}
