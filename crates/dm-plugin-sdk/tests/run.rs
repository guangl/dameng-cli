use dm_plugin_sdk::{Context, Plugin, PluginResult, run};

struct ExitZero;

impl Plugin for ExitZero {
    fn run(&self, _context: Context) -> PluginResult {
        Ok(0)
    }
}

#[test]
fn run_exits_with_plugin_code() {
    for (key, value) in [
        ("DM_PLUGIN_API_VERSION", "1"),
        ("DM_PLUGIN_CAPABILITIES", "config-dirs-v1"),
        ("DM_PLUGIN_DIR", "/tmp/dm-sdk-plugin"),
        ("DM_PLUGIN_HOME", "/tmp/dm-sdk-home"),
        ("DM_PLUGIN_CONFIG_DIR", "/tmp/dm-sdk-config"),
        ("DM_PLUGIN_DATA_DIR", "/tmp/dm-sdk-data"),
        ("DM_PLUGIN_CACHE_DIR", "/tmp/dm-sdk-cache"),
    ] {
        unsafe {
            std::env::set_var(key, value);
        }
    }
    run(ExitZero);
}
