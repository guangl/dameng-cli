//! Plugin directory layout, completion and purge behaviour.
use crate::common::*;
use crate::legacy::*;
use dameng_cli::PluginStore;
use std::{fs, path::Path};
use tempfile::TempDir;

#[test]
fn plugin_directories_are_grouped_per_plugin() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let store = PluginStore::new(&home);
    let [config, data, cache] = store.plugin_directories("probe");
    // Compare component-wise: Windows reports backslash separators.
    assert_eq!(config, home.join("probe").join("config"));
    assert_eq!(data, home.join("probe").join("data"));
    assert_eq!(cache, home.join("probe").join("cache"));

    let source = fixture(temp.path());
    store.install(source.to_str().unwrap()).unwrap();
    let info = ok(dm(&home)
        .args(["info", "probe", "--json"])
        .output()
        .unwrap());
    let info: serde_json::Value = serde_json::from_str(&info).unwrap();
    let reported = Path::new(info["paths"]["data"].as_str().unwrap());
    assert!(
        reported.ends_with(Path::new("probe").join("data")),
        "{reported:?}"
    );
    let config_file = Path::new(info["paths"]["config_file"].as_str().unwrap());
    assert!(
        config_file.ends_with(Path::new("probe").join("config").join("config.toml")),
        "{config_file:?}"
    );
}

/// A plugin that reports the directories the host handed it.
#[cfg(unix)]
fn reporting_package(root: &Path) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let source = root.join("reporting package");
    fs::create_dir_all(&source).unwrap();
    fs::write(
        source.join("dm-plugin.toml"),
        format!("{}completion = true\n", manifest("probe")),
    )
    .unwrap();
    let binary = source.join("dm-probe");
    fs::write(
        &binary,
        "#!/bin/sh\necho \"config=$DM_PLUGIN_CONFIG_DIR\"\necho prod\n",
    )
    .unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
    source
}

#[cfg(unix)]
#[test]
fn completion_reads_the_legacy_directory_before_migration() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let store = PluginStore::new(&home);
    let source = reporting_package(temp.path());
    store.install(source.to_str().unwrap()).unwrap();
    let legacy = home.join("config").join("probe");
    fs::create_dir_all(&legacy).unwrap();
    fs::write(legacy.join("config.toml"), "greeting = \"hi\"\n").unwrap();

    let candidates = store.complete_plugin("probe", &["".into()]).unwrap();
    let reported = candidates
        .iter()
        .find_map(|line| line.strip_prefix("config="))
        .unwrap_or_else(|| panic!("no config directory in {candidates:?}"));
    assert!(
        Path::new(reported).ends_with(Path::new("config").join("probe")),
        "{reported}"
    );
    // Completion is read-only: it must not migrate while answering.
    assert!(legacy.is_dir());
    assert!(!home.join("probe").exists());
}

#[test]
fn purge_removes_legacy_directories_without_running_the_plugin() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let store = PluginStore::new(&home);
    let source = fixture(temp.path());
    store.install(source.to_str().unwrap()).unwrap();
    for (kind, file) in LEGACY_PLUGIN_DIRECTORIES {
        let legacy = home.join(kind).join("probe");
        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join(file), "kept").unwrap();
    }

    ok(dm(&home)
        .args(["uninstall", "probe", "--purge", "--yes"])
        .output()
        .unwrap());
    for legacy in legacy_plugin_directories(&home) {
        assert!(!legacy.exists(), "{}", legacy.display());
    }
    for path in store.removal_paths("probe") {
        assert!(!path.exists(), "{}", path.display());
    }
}

#[test]
fn doctor_removes_directories_of_plugins_that_are_gone() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let store = PluginStore::new(&home);
    fs::create_dir_all(home.join("ghost/data")).unwrap();
    fs::write(home.join("ghost/data/saved"), "leftover").unwrap();

    let report = store.doctor(false).unwrap();
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.contains("orphaned plugin directory: ghost")),
        "{report:?}"
    );
    let repaired = store.doctor(true).unwrap();
    assert!(
        repaired
            .repairs
            .iter()
            .any(|repair| repair.contains("ghost")),
        "{repaired:?}"
    );
    assert!(!home.join("ghost").exists());
}
