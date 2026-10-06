//! Regression coverage for reserved recovery review findings.
#![cfg(unix)]
use crate::{common::*, plugin_names::install_legacy_plugin};
use dameng_cli::PluginStore;
use std::{
    fs,
    sync::{Arc, Barrier},
};
use tempfile::TempDir;

fn setup() -> (TempDir, std::path::PathBuf) {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    install_legacy_plugin(&home, "data");
    for kind in ["config", "data", "cache"] {
        let source = home.join("data").join(kind);
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("keep"), kind).unwrap();
    }
    (temp, home)
}

#[test]
fn recovered_reserved_metadata_preserves_grouped_data_across_repairs() {
    let (_temp, home) = setup();
    rusqlite::Connection::open(home.join("store.sqlite3"))
        .unwrap()
        .execute("DELETE FROM installed_plugins WHERE name = 'data'", [])
        .unwrap();
    let store = PluginStore::new(&home);
    for _ in 0..2 {
        store.doctor(true).unwrap();
        store.info("data").unwrap();
        for kind in ["config", "data", "cache"] {
            assert_eq!(
                fs::read_to_string(home.join(kind).join("data/keep")).unwrap(),
                kind
            );
        }
    }
}

#[test]
fn dangling_destination_symlink_is_preserved() {
    let (_temp, home) = setup();
    fs::create_dir_all(home.join("config")).unwrap();
    let target = home.join("config/data");
    std::os::unix::fs::symlink(home.join("missing"), &target).unwrap();
    let output = dm(&home).arg("data").output().unwrap();
    assert!(!output.status.success());
    assert!(stderr(&output).contains("non-directory"));
    assert_eq!(fs::read_link(&target).unwrap(), home.join("missing"));
    for kind in ["config", "data", "cache"] {
        assert!(home.join("data").join(kind).join("keep").is_file());
    }
}

#[test]
fn concurrent_reserved_launches_all_succeed() {
    let (_temp, home) = setup();
    for kind in ["config", "data", "cache"] {
        fs::create_dir_all(home.join(kind).join("data")).unwrap();
    }
    let barrier = Arc::new(Barrier::new(12));
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..12)
            .map(|_| {
                let barrier = barrier.clone();
                let home = &home;
                scope.spawn(move || {
                    barrier.wait();
                    ok(dm(home).arg("data").output().unwrap());
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }
    });
    for kind in ["config", "data", "cache"] {
        assert_eq!(
            fs::read_to_string(home.join(kind).join("data/keep")).unwrap(),
            kind
        );
    }
}
