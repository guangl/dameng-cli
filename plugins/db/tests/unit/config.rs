//! Tests for the plugin's own config.toml.

use crate::common::*;
use dm_plugin_db::{DbConfig, config_path, load_config};
use std::fs;
use tempfile::TempDir;

#[test]
fn plugin_config_is_optional_and_validated() {
    let temp = TempDir::new().unwrap();
    let context = context(&temp);

    assert_eq!(
        config_path(&context),
        context.config_dir.join("config.toml")
    );
    assert_eq!(load_config(&context).unwrap(), DbConfig::default());

    fs::create_dir_all(&context.config_dir).unwrap();
    fs::write(
        config_path(&context),
        concat!(
            "[defaults]\nport = 5237\nusername = \"DMHR\"\ndriver = \"DM8 ODBC DRIVER\"\n",
            "schema = \"DMHR\"\n\n[connect]\ntimeout = 3\nprobe = \"SELECT 1 FROM DUAL\"\n",
        ),
    )
    .unwrap();
    let config = load_config(&context).unwrap();
    assert_eq!(config.defaults.port, Some(5237));
    assert_eq!(config.defaults.username.as_deref(), Some("DMHR"));
    assert_eq!(config.defaults.schema.as_deref(), Some("DMHR"));
    assert_eq!(config.connect.timeout, Some(3));
    assert_eq!(config.connect.probe.as_deref(), Some("SELECT 1 FROM DUAL"));

    for (text, expected) in [
        ("[defaults]\nport = 0\n", "defaults.port"),
        ("[connect]\ntimeout = 0\n", "greater than zero"),
        ("[defaults]\nusername = \"  \"\n", "must not be empty"),
        ("[defaults]\ndriver = \"\"\n", "must not be empty"),
        ("[defaults]\nschema = \" \"\n", "must not be empty"),
        ("[connect]\nprobe = \"\"\n", "must not be empty"),
        (
            "[connect]\nlibrary = \"/opt/dm/libdmdodbc.so\"\n",
            "Invalid database plugin configuration",
        ),
        (
            "[nope]\nport = 1\n",
            "Invalid database plugin configuration",
        ),
        ("port = 1\n", "Invalid database plugin configuration"),
    ] {
        fs::write(config_path(&context), text).unwrap();
        let error = load_config(&context).unwrap_err();
        assert!(
            format!("{error:#}").contains(expected),
            "{text} -> {error:#}"
        );
    }
}

#[test]
fn shipped_example_config_is_accepted() {
    let temp = TempDir::new().unwrap();
    let context = context(&temp);
    fs::create_dir_all(&context.config_dir).unwrap();
    let example = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config.example.toml");
    fs::write(config_path(&context), fs::read_to_string(&example).unwrap()).unwrap();

    let config = load_config(&context).unwrap();
    assert_eq!(config.defaults.port, Some(5236));
    assert_eq!(config.defaults.username.as_deref(), Some("SYSDBA"));
    assert_eq!(config.defaults.driver.as_deref(), Some("DM8 ODBC DRIVER"));
    assert_eq!(config.connect.timeout, Some(10));
    assert_eq!(config.connect.probe.as_deref(), Some("SELECT 1"));
}

#[test]
fn unreadable_plugin_configuration_is_reported() {
    let temp = TempDir::new().unwrap();
    let context = context(&temp);
    // A directory in place of the file makes reading it fail.
    fs::create_dir_all(config_path(&context)).unwrap();
    let error = load_config(&context).unwrap_err();
    assert!(format!("{error:#}").contains("config.toml"), "{error:#}");
}
