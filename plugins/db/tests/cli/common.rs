//! Helpers shared by the end-to-end tests: the plugin command and its outputs.

use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

pub(crate) fn db(home: &Path) -> Command {
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

pub(crate) fn ok(output: Output) -> String {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

pub(crate) fn failure(output: Output) -> String {
    assert!(!output.status.success(), "expected a failing command");
    String::from_utf8_lossy(&output.stderr).into_owned()
}

pub(crate) fn write_plugin_config(home: &Path, text: &str) {
    let directory = home.join("config/db");
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join("config.toml"), text).unwrap();
}
