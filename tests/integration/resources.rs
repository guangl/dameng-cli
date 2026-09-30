use crate::common::*;
use std::fs;
use tempfile::TempDir;

#[test]
fn resource_configuration_obeys_environment_precedence_and_rejects_invalid_limits() {
    let temp = TempDir::new().unwrap();
    write_config(temp.path(), "[update]\ncheck_concurrency = 2\n");
    for (override_value, expected, source) in [
        (None, 2, "config"),
        (Some("1"), 1, "env:DM_UPDATE_CHECK_CONCURRENCY"),
    ] {
        let mut command = dm_isolated(temp.path());
        if let Some(value) = override_value {
            command.env("DM_UPDATE_CHECK_CONCURRENCY", value);
        }
        let output = ok(command.args(["config", "show", "--json"]).output().unwrap());
        let value: serde_json::Value = serde_json::from_str(&output).unwrap();
        let entry = value["settings"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["key"] == "update.check_concurrency")
            .unwrap();
        assert_eq!(entry["value"], expected);
        assert_eq!(entry["source"], source);
    }
    for value in ["0", "17", "invalid"] {
        let output = dm_isolated(temp.path())
            .env("DM_UPDATE_CHECK_CONCURRENCY", value)
            .args(["list"])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(stderr(&output).contains("concurrency") || stderr(&output).contains("CONCURRENCY"));
    }
}

#[test]
fn oversized_host_configuration_is_rejected_without_parsing_it() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("config.toml");
    let file = fs::File::create(path).unwrap();
    file.set_len(1024 * 1024 + 1).unwrap();
    let output = dm_isolated(temp.path()).args(["list"]).output().unwrap();
    assert!(!output.status.success());
    assert!(stderr(&output).contains("exceeds 1048576 bytes"));
}
