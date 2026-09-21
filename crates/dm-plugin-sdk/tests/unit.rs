use std::{path::PathBuf, sync::Mutex};

use dm_plugin_sdk::{Context, Plugin, PluginResult, required_path, run_code};

static LOCK: Mutex<()> = Mutex::new(());

fn set(name: &str, value: &str) {
    unsafe {
        std::env::set_var(name, value);
    }
}

fn remove(name: &str) {
    unsafe {
        std::env::remove_var(name);
    }
}

fn configure_valid_env() {
    set("DM_PLUGIN_API_VERSION", "1");
    set("DM_PLUGIN_CAPABILITIES", "config-dirs-v1");
    set("DM_PLUGIN_DIR", "/tmp/plugin");
    set("DM_PLUGIN_HOME", "/tmp/home");
    set("DM_PLUGIN_CONFIG_DIR", "/tmp/config");
    set("DM_PLUGIN_DATA_DIR", "/tmp/data");
    set("DM_PLUGIN_CACHE_DIR", "/tmp/cache");
}

#[test]
fn required_path_reads_env() {
    let _guard = LOCK.lock().unwrap();
    set("DM_TEST_PATH", "/tmp/dm-test");
    assert_eq!(
        required_path("DM_TEST_PATH").unwrap(),
        PathBuf::from("/tmp/dm-test")
    );
    remove("DM_TEST_PATH");
    assert!(required_path("DM_TEST_PATH").is_err());
}

#[test]
fn context_from_env_success() {
    let _guard = LOCK.lock().unwrap();
    configure_valid_env();
    let context = Context::from_env().unwrap();
    assert_eq!(context.plugin_dir, PathBuf::from("/tmp/plugin"));
    assert_eq!(context.home, PathBuf::from("/tmp/home"));
    assert_eq!(context.capabilities, vec!["config-dirs-v1"]);
}

#[test]
fn context_requires_host_and_capabilities() {
    let _guard = LOCK.lock().unwrap();
    remove("DM_PLUGIN_API_VERSION");
    assert!(Context::from_env().is_err());
    set("DM_PLUGIN_API_VERSION", "999");
    assert!(Context::from_env().is_err());
    set("DM_PLUGIN_API_VERSION", "1");
    remove("DM_PLUGIN_CAPABILITIES");
    assert!(Context::from_env().is_err());
    set("DM_PLUGIN_CAPABILITIES", "other");
    assert!(Context::from_env().is_err());
}

#[test]
fn context_requires_per_plugin_directories() {
    let _guard = LOCK.lock().unwrap();
    set("DM_PLUGIN_API_VERSION", "1");
    set("DM_PLUGIN_CAPABILITIES", "config-dirs-v1");
    set("DM_PLUGIN_DIR", "/tmp/plugin");
    set("DM_PLUGIN_HOME", "/tmp/home");
    remove("DM_PLUGIN_CONFIG_DIR");
    assert!(Context::from_env().is_err());
    set("DM_PLUGIN_CONFIG_DIR", "/tmp/config");
    remove("DM_PLUGIN_DATA_DIR");
    assert!(Context::from_env().is_err());
    set("DM_PLUGIN_DATA_DIR", "/tmp/data");
    remove("DM_PLUGIN_CACHE_DIR");
    assert!(Context::from_env().is_err());
}

struct CodePlugin(i32);

impl Plugin for CodePlugin {
    fn run(&self, _context: Context) -> PluginResult {
        Ok(self.0)
    }
}

struct FailPlugin;

impl Plugin for FailPlugin {
    fn run(&self, _context: Context) -> PluginResult {
        Err("boom".into())
    }
}

#[test]
fn run_code_preserves_plugin_exit_code() {
    let _guard = LOCK.lock().unwrap();
    configure_valid_env();
    assert_eq!(run_code(CodePlugin(7)), 7);
}

#[test]
fn run_code_reports_errors_and_returns_one() {
    let _guard = LOCK.lock().unwrap();
    configure_valid_env();
    assert_eq!(run_code(FailPlugin), 1);
}
