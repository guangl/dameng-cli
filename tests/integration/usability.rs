use crate::common::*;
use dameng_cli::PluginStore;
use std::fs;
use tempfile::TempDir;

#[test]
fn uninstall_keeps_connections_across_repair_and_reinstall() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    let store = PluginStore::new(&home);
    store.install(source.to_str().unwrap()).unwrap();
    for path in store.plugin_directories("probe") {
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("saved"), "connection").unwrap();
    }
    ok(dm(&home).args(["uninstall", "probe"]).output().unwrap());
    store.doctor(true).unwrap();
    for path in store.plugin_directories("probe") {
        assert!(path.join("saved").is_file());
    }
    store.install(source.to_str().unwrap()).unwrap();
    let cancelled = dm(&home)
        .args(["uninstall", "probe", "--purge"])
        .output()
        .unwrap();
    assert!(!cancelled.status.success());
    assert!(store.info("probe").is_ok());
    ok(dm(&home)
        .args(["uninstall", "probe", "--purge", "--yes"])
        .output()
        .unwrap());
    for path in store.plugin_directories("probe") {
        assert!(!path.exists());
    }
}
#[test]
fn completion_does_not_create_home_or_log_and_ignores_invalid_configuration() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    assert!(ok(dm(&home).args(["complete", "--", "do"]).output().unwrap()).contains("doctor"));
    assert!(!home.exists());
    fs::create_dir_all(&home).unwrap();
    fs::write(home.join("config.toml"), "invalid").unwrap();
    assert!(ok(dm(&home).args(["complete", "--", "co"]).output().unwrap()).contains("config"));
    assert!(!home.join("logs").exists());
    assert!(!home.join("store.sqlite3").exists());
}
#[test]
fn host_config_reports_environment_precedence_without_secrets() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    ok(dm(&home).args(["config", "init"]).output().unwrap());
    assert!(
        !dm(&home)
            .args(["config", "init"])
            .output()
            .unwrap()
            .status
            .success()
    );
    let output = ok(dm(&home)
        .env("DM_PROGRESS", "false")
        .args(["config", "show", "--json"])
        .output()
        .unwrap());
    let settings: serde_json::Value = serde_json::from_str(&output).unwrap();
    let progress = settings["settings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["key"] == "output.progress")
        .unwrap();
    assert_eq!(progress["source"], "env:DM_PROGRESS");
    assert_eq!(progress["value"], false);
    assert!(ok(dm(&home).args(["config", "path"]).output().unwrap()).contains("config.toml"));
    assert!(ok(dm(&home).args(["config", "show"]).output().unwrap()).contains("log.level"));
}
#[test]
fn all_shell_scripts_use_the_runtime_protocol() {
    let temp = TempDir::new().unwrap();
    for shell in ["bash", "zsh"] {
        let script = ok(dm(temp.path())
            .args(["completions", shell])
            .output()
            .unwrap());
        assert!(script.contains("dm complete --"), "{shell}");
    }
    for removed in ["fish", "elvish", "powershell"] {
        let output = dm(temp.path())
            .args(["completions", removed])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("bash, zsh"));
    }
}
#[cfg(unix)]
#[test]
fn completion_only_runs_opted_in_plugins_and_times_out_without_noise() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("package");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("dm-plugin.toml"), manifest("probe")).unwrap();
    let binary = source.join("dm-probe");
    fs::write(
        &binary,
        "#!/bin/sh\necho prod\necho stage\necho ignored >&2\n",
    )
    .unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
    let home = temp.path().join("home");
    let store = PluginStore::new(&home);
    store.install(source.to_str().unwrap()).unwrap();
    assert!(
        store
            .complete_plugin("probe", &["".into()])
            .unwrap()
            .is_empty()
    );
    fs::write(
        source.join("dm-plugin.toml"),
        format!("{}completion = true\n", manifest("probe")),
    )
    .unwrap();
    store
        .install_with_revision(source.to_str().unwrap(), None, true)
        .unwrap();
    for path in store.plugin_directories("probe") {
        if path.exists() {
            fs::remove_dir_all(path).unwrap();
        }
    }
    assert_eq!(
        store.complete_plugin("probe", &["pr".into()]).unwrap(),
        ["prod", "stage"]
    );
    let out = dm(&home)
        .args(["complete", "--", "probe", "pr"])
        .output()
        .unwrap();
    assert_eq!(ok(out.clone()).trim(), "prod");
    assert!(out.stderr.is_empty());
    assert!(!home.join("data/probe").exists());
    assert_eq!(
        ok(dm(&home).args(["complete", "--", "pr"]).output().unwrap()).trim(),
        "probe"
    );
    assert_eq!(
        ok(dm(&home)
            .args(["complete", "--", "update", "pr"])
            .output()
            .unwrap())
        .trim(),
        "probe"
    );
    fs::write(&binary, "#!/bin/sh\nexec sleep 5\n").unwrap();
    store
        .install_with_revision(source.to_str().unwrap(), None, true)
        .unwrap();
    let start = std::time::Instant::now();
    assert!(
        store
            .complete_plugin("probe", &["".into()])
            .unwrap()
            .is_empty()
    );
    assert!(start.elapsed().as_secs_f64() < 2.0);
    fs::write(&binary, "#!/bin/sh\nexit 2\n").unwrap();
    store
        .install_with_revision(source.to_str().unwrap(), None, true)
        .unwrap();
    assert!(
        store
            .complete_plugin("probe", &["".into()])
            .unwrap()
            .is_empty()
    );
}
