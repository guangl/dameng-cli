use std::fs;

use dm_plugin_ssh::{SshConfig, config_path, load_config};
use tempfile::TempDir;

use crate::common::*;

#[test]
fn plugin_config_is_optional_and_validated() {
    let temp = TempDir::new().unwrap();
    let context = context(&temp);

    // Without a file every value falls back to the plugin default.
    assert_eq!(
        config_path(&context),
        context.config_dir.join("config.toml")
    );
    assert_eq!(load_config(&context).unwrap(), SshConfig::default());

    fs::create_dir_all(&context.config_dir).unwrap();
    fs::write(
        config_path(&context),
        concat!(
            "[defaults]\nport = 2200\nusername = \"ubuntu\"\n",
            "auth = \"key\"\nkey = \"/tmp/id\"\n\n[test]\nconnect_timeout = 3\n",
        ),
    )
    .unwrap();
    let config = load_config(&context).unwrap();
    assert_eq!(config.defaults.port, Some(2200));
    assert_eq!(config.defaults.username.as_deref(), Some("ubuntu"));
    assert_eq!(config.defaults.auth.as_deref(), Some("key"));
    assert_eq!(config.test.connect_timeout, Some(3));

    for (text, expected) in [
        ("[defaults]\nauth = \"token\"\n", "defaults.auth"),
        ("[test]\nconnect_timeout = 0\n", "greater than zero"),
        ("[defaults]\nusername = \"  \"\n", "must not be empty"),
        ("[defaults]\nkey = \"\"\n", "must not be empty"),
        ("[nope]\nport = 1\n", "Invalid SSH plugin configuration"),
        ("port = 1\n", "Invalid SSH plugin configuration"),
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
    assert_eq!(config.defaults.port, Some(22));
    assert_eq!(config.test.connect_timeout, Some(10));
}
