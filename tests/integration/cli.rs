//! CLI surface: JSON output, empty states and reported paths.

use crate::common::*;
use dameng_cli::PluginStore;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn json_cli_reports_manifest_name() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    ok(dm(&home)
        .args(["install", source.to_str().unwrap()])
        .output()
        .unwrap());
    let output = ok(dm(&home).args(["list", "--json"]).output().unwrap());
    let json: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(json[0]["manifest"]["name"], "probe");
    assert!(ok(dm(&home).args(["completions", "bash"]).output().unwrap()).contains("_dm"));
}
#[test]
fn cli_reporting_branches_cover_info_update_doctor() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    fs::write(
        source.join("dm-plugin.toml"),
        format!(
            r#"{}environment = ["DM_DATABASE_URL"]
"#,
            manifest("probe")
        ),
    )
    .unwrap();
    let home = temp.path().join("home");
    ok(dm(&home)
        .args(["install", source.to_str().unwrap()])
        .output()
        .unwrap());

    let info = ok(dm(&home).args(["info", "probe"]).output().unwrap());
    assert!(info.contains("名称： probe"), "{info}");
    assert!(info.contains("版本： 0.1.0"), "{info}");
    assert!(info.contains("继承环境变量： DM_DATABASE_URL"), "{info}");
    let info_json = ok(dm(&home)
        .args(["info", "--json", "probe"])
        .output()
        .unwrap());
    let parsed: serde_json::Value = serde_json::from_str(&info_json).unwrap();
    assert_eq!(parsed["manifest"]["name"], "probe");

    assert_eq!(
        PluginStore::new(&home).verify(Some("probe")).unwrap(),
        ["probe"]
    );

    ok(dm(&home).args(["update", "probe"]).output().unwrap());

    fs::write(
        source.join("dm-plugin.toml"),
        format!(
            r#"{}environment = ["DM_DATABASE_URL"]
"#,
            manifest("probe").replace("0.1.0", "0.2.0")
        ),
    )
    .unwrap();
    let cargo = fs::read_to_string(source.join("Cargo.toml")).unwrap();
    fs::write(source.join("Cargo.toml"), cargo.replace("0.1.0", "0.2.0")).unwrap();
    ok(Command::new("cargo")
        .args(["generate-lockfile", "--offline", "--manifest-path"])
        .arg(source.join("Cargo.toml"))
        .output()
        .unwrap());
    ok(dm(&home).args(["update", "probe"]).output().unwrap());

    let healthy = ok(dm(&home).args(["doctor"]).output().unwrap());
    assert!(healthy.contains("插件存储正常"), "{healthy}");
    let doctor_json = ok(dm(&home).args(["doctor", "--json"]).output().unwrap());
    let doctor: serde_json::Value = serde_json::from_str(&doctor_json).unwrap();
    assert!(doctor["issues"].is_array());

    let orphan = home.join("config").join("orphan");
    fs::create_dir_all(&orphan).unwrap();
    fs::write(orphan.join("leftover"), "leftover").unwrap();
    let issues = ok(dm(&home).args(["doctor"]).output().unwrap());
    assert!(issues.contains("问题："), "{issues}");
    assert!(issues.contains("运行 dm doctor --repair"), "{issues}");
    let repaired = ok(dm(&home).args(["doctor", "--repair"]).output().unwrap());
    assert!(repaired.contains("已修复："), "{repaired}");
    assert!(!orphan.exists());

    fs::remove_file(source.join(format!("dm-probe{}", std::env::consts::EXE_SUFFIX))).unwrap();
    let failed = dm(&home).args(["update", "--all"]).output().unwrap();
    assert!(!failed.status.success());
    assert!(
        String::from_utf8_lossy(&failed.stderr).contains("Some updates failed"),
        "{}",
        String::from_utf8_lossy(&failed.stderr)
    );
}
#[test]
fn relative_dm_home_is_resolved_against_current_dir() {
    let temp = TempDir::new().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_dm"))
        .current_dir(temp.path())
        .env("DM_PLUGIN_HOME", "relhome")
        .args(["list"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(temp.path().join("relhome/store.sqlite3").is_file());
}
#[test]
fn empty_state_messages_cover_update_and_version_checks() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");

    let update = ok(dm(&home).args(["update", "--all"]).output().unwrap());
    assert!(update.contains("尚无插件"), "{update}");
    let outdated = ok(dm(&home).args(["update"]).output().unwrap());
    assert!(outdated.contains("尚无插件"), "{outdated}");
    let json = ok(dm(&home).args(["update", "--json"]).output().unwrap());
    assert_eq!(json.trim(), "[]");
}
#[test]
fn install_checks_the_published_plugin_integrity() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    let store = PluginStore::new(&home);
    store.install(source.to_str().unwrap()).unwrap();

    assert_eq!(store.verify(None).unwrap(), ["probe"]);
}
#[test]
fn info_points_at_the_plugin_owned_configuration() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    let store = PluginStore::new(&home);
    store.install(source.to_str().unwrap()).unwrap();

    let info = ok(dm(&home).args(["info", "probe"]).output().unwrap());
    assert!(info.contains("配置目录："), "{info}");
    assert!(info.contains("数据目录："), "{info}");
    assert!(info.contains("缓存目录："), "{info}");
    assert!(info.contains("(不存在)"), "{info}");

    // The plugin owns this file; the host only reports where it belongs.
    fs::create_dir_all(home.join("config/probe")).unwrap();
    fs::write(home.join("config/probe/config.toml"), "greeting = \"hi\"\n").unwrap();
    let info = ok(dm(&home).args(["info", "probe"]).output().unwrap());
    assert!(info.contains("(存在)"), "{info}");

    let json = ok(dm(&home)
        .args(["info", "--json", "probe"])
        .output()
        .unwrap());
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["paths"]["config_file_present"], true);
    // Compare paths component-wise: Windows reports backslash separators.
    let config = Path::new(parsed["paths"]["config"].as_str().unwrap());
    assert!(
        config.ends_with(Path::new("config").join("probe")),
        "{config:?}"
    );
    let config_file = Path::new(parsed["paths"]["config_file"].as_str().unwrap());
    assert!(
        config_file.ends_with(Path::new("config").join("probe").join("config.toml")),
        "{config_file:?}"
    );
}

#[test]
fn version_flags_work_without_valid_configuration_or_storage() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    fs::write(home.join("config.toml"), "invalid = [").unwrap();
    for flag in ["--version", "-V"] {
        let output = ok(dm(&home).arg(flag).output().unwrap());
        assert_eq!(output.trim(), concat!("dm ", env!("CARGO_PKG_VERSION")));
    }
    assert!(!home.join("store.sqlite3").exists());
    assert!(!home.join("logs").exists());
}
