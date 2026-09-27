//! Unit tests for the hints `dm` prints when a command fails.

use dameng_cli::cli::report::hint_for;

fn hint_for_message(message: &str) -> String {
    hint_for(&anyhow::anyhow!(message.to_owned()))
}

#[test]
fn known_failures_get_specific_hints() {
    assert!(hint_for_message("Plugin 'x' is not installed").contains("dm install"));
    assert!(
        hint_for_message("Plugin 'x' is already installed; run dm update").contains("--replace")
    );
    assert!(hint_for_message("Plugin source is not updateable").contains("--replace"));
    assert!(
        hint_for_message("Local plugin package has no dm-probe binary; build the plugin")
            .contains("cargo build --release --locked")
    );
    assert!(hint_for_message("Source must be a local plugin directory").contains("<source>"));
    assert!(hint_for_message("directory is not writable").contains("DM_PLUGIN_HOME"));
    assert!(hint_for_message("Prebuilt plugin SHA-256 mismatch").contains("重新下载"));
    assert!(
        hint_for_message("Invalid configuration /home/me/.config/dm/config.toml: unknown field")
            .contains("config.toml")
    );
    // The previous flat layout points at the table that replaced it.
    assert!(
        hint_for_message("Invalid configuration /x/config.toml: unknown field `update_repository`")
            .contains("[update]")
    );
    assert!(
        hint_for_message("Self-update repository 'x' must be in owner/repository form")
            .contains("owner/repository")
    );
    assert!(hint_for_message("stale transaction directory: .install-x").contains("dm doctor"));
    assert!(hint_for_message("Unsupported plugin API version 2").contains("API 版本"));
    assert!(hint_for_message("Invalid manifest /x/dm-plugin.toml").contains("dm-plugin.toml"));
    assert!(hint_for_message("Self-update release asset is missing").contains("自更新失败"));
    assert!(hint_for_message("Invalid plugin name").contains("插件名"));
    assert!(
        hint_for_message("Invalid DM_PLUGIN_ENVIRONMENT; use a comma-separated list")
            .contains("[plugin] environment")
    );
    assert!(
        hint_for_message("Configuration key 'plugin.environment' entry 'DB-URL' is invalid")
            .contains("[plugin] environment")
    );
    assert!(
        hint_for_message("DM_PROGRESS must be true or false, got 'maybe'").contains("true/false")
    );
    assert!(
        hint_for_message("Self-update is not published for target x; supported targets are y")
            .contains("--target")
    );
}

#[test]
fn unknown_failures_still_get_a_hint() {
    assert!(hint_for_message("unexpected failure").contains("dm --help"));
}
