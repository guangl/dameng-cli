use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

#[allow(dead_code)]
pub fn ssh(home: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_dm-ssh"));
    command
        .env("DM_PLUGIN_API_VERSION", "1")
        .env("DM_PLUGIN_CAPABILITIES", "config-dirs-v1")
        .env("DM_PLUGIN_DIR", home.join("plugins/ssh"))
        .env("DM_PLUGIN_HOME", home)
        .env("DM_PLUGIN_CONFIG_DIR", home.join("config/ssh"))
        .env("DM_PLUGIN_DATA_DIR", home.join("data/ssh"))
        .env("DM_PLUGIN_CACHE_DIR", home.join("cache/ssh"));
    command
}

#[allow(dead_code)]
pub fn ok(output: Output) -> String {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[allow(dead_code)]
pub fn failure(output: Output) -> String {
    assert!(!output.status.success(), "expected a failing command");
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[allow(dead_code)]
/// The plugin owns its configuration inside the directory the host passes.
pub fn write_plugin_config(home: &Path, text: &str) {
    let directory = home.join("config/ssh");
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join("config.toml"), text).unwrap();
}
