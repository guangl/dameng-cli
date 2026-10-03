//! Migration from the older per-kind plugin directories.
use crate::common::*;
use crate::legacy::*;
use dameng_cli::PluginStore;
use std::{fs, path::Path};
use tempfile::TempDir;

#[test]
fn running_a_plugin_migrates_the_older_layout_once() {
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

    let output = ok(dm(&home).args(["probe"]).output().unwrap());
    let reported = reported_data_dir(&output);
    assert!(
        reported.ends_with(Path::new("probe").join("data")),
        "{reported:?}"
    );
    for (kind, file) in LEGACY_PLUGIN_DIRECTORIES {
        let moved = home.join("probe").join(kind).join(file);
        assert_eq!(
            fs::read_to_string(&moved).unwrap(),
            "kept",
            "{}",
            moved.display()
        );
    }
    for legacy in legacy_plugin_directories(&home) {
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
fn migration_keeps_data_when_the_destination_is_an_empty_placeholder() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let store = PluginStore::new(&home);
    let source = fixture(temp.path());
    store.install(source.to_str().unwrap()).unwrap();
    // Something else (for example a configured log directory) created the path
    // first; the plugin's own data must still move into it.
    fs::create_dir_all(home.join("probe/data")).unwrap();
    let legacy = home.join("data/probe");
    fs::create_dir_all(&legacy).unwrap();
    fs::write(legacy.join("connections.sqlite3"), "kept").unwrap();

    ok(dm(&home).args(["probe"]).output().unwrap());
    assert_eq!(
        fs::read_to_string(home.join("probe/data/connections.sqlite3")).unwrap(),
        "kept"
    );
    assert!(!legacy.exists());
}
