//! Plugin directory layout and the migration from the older layout.
use crate::common::*;
use dameng_cli::PluginStore;
use std::{fs, path::Path};
use tempfile::TempDir;

const LEGACY: [(&str, &str); 3] = [
    ("config", "config.toml"),
    ("data", "connections.sqlite3"),
    ("cache", "session"),
];

/// The fixture plugin prints the data directory it was handed.
fn reported_data_dir(output: &str) -> std::path::PathBuf {
    let line = output
        .lines()
        .find(|line| line.starts_with("data="))
        .unwrap_or_else(|| panic!("no data directory in {output}"));
    std::path::PathBuf::from(line.trim_start_matches("data="))
}

fn legacy_directories(home: &std::path::Path) -> Vec<std::path::PathBuf> {
    LEGACY
        .iter()
        .map(|(kind, _)| home.join(kind).join("probe"))
        .collect()
}

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

#[test]
fn running_a_plugin_migrates_the_older_layout_once() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let store = PluginStore::new(&home);
    let source = fixture(temp.path());
    store.install(source.to_str().unwrap()).unwrap();
    for (kind, file) in LEGACY {
        let legacy = home.join(kind).join("probe");
        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join(file), "kept").unwrap();
    }

    let output = ok(dm(&home).args(["probe"]).output().unwrap());
    let reported = reported_data_dir(&output);
    assert!(
        reported.ends_with(Path::new("probe").join("data")),
        "{reported:?}"
    );
    for (kind, file) in LEGACY {
        let moved = home.join("probe").join(kind).join(file);
        assert_eq!(
            fs::read_to_string(&moved).unwrap(),
            "kept",
            "{}",
            moved.display()
        );
    }
    for legacy in legacy_directories(&home) {
        assert!(!legacy.exists(), "{}", legacy.display());
    }

    // The migration is idempotent, and later runs only see the new layout.
    let second = ok(dm(&home).args(["probe"]).output().unwrap());
    let reported = reported_data_dir(&second);
    assert!(
        reported.ends_with(Path::new("probe").join("data")),
        "{reported:?}"
    );
}

#[test]
fn existing_destinations_win_over_leftover_legacy_directories() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let store = PluginStore::new(&home);
    let source = fixture(temp.path());
    store.install(source.to_str().unwrap()).unwrap();
    fs::create_dir_all(home.join("probe/data")).unwrap();
    fs::write(home.join("probe/data/current"), "current").unwrap();
    let legacy = home.join("data/probe");
    fs::create_dir_all(&legacy).unwrap();
    fs::write(legacy.join("stale"), "stale").unwrap();

    ok(dm(&home).args(["probe"]).output().unwrap());
    assert!(home.join("probe/data/current").is_file());
    assert!(
        legacy.join("stale").is_file(),
        "legacy data is left untouched"
    );
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
