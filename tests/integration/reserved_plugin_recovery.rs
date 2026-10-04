//! Recovery after the preceding release grouped a legacy reserved-name plugin.
#![cfg(unix)]
use crate::{common::*, plugin_names::install_legacy_plugin};
use dameng_cli::PluginStore;
use std::fs;
use tempfile::TempDir;

fn grouped_data(home: &std::path::Path, name: &str) {
    for kind in ["config", "data", "cache"] {
        let directory = home.join(name).join(kind);
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("keep"), format!("{name}-{kind}")).unwrap();
    }
}

#[test]
fn running_restores_already_grouped_reserved_plugins() {
    for name in ["data", "cache", "logs", "plugins", "backups"] {
        let temp = TempDir::new().unwrap();
        let home = temp.path().join("home");
        fs::create_dir_all(&home).unwrap();
        install_legacy_plugin(&home, name);
        grouped_data(&home, name);
        // An earlier launch of the protection release may leave placeholders.
        for kind in ["config", "data", "cache"] {
            fs::create_dir_all(home.join(kind).join(name)).unwrap();
        }
        let output = ok(dm(&home).arg(name).output().unwrap());
        for kind in ["config", "data", "cache"] {
            let target = home.join(kind).join(name);
            assert!(output.contains(target.to_str().unwrap()), "{output}");
            assert_eq!(
                fs::read_to_string(target.join("keep")).unwrap(),
                format!("{name}-{kind}")
            );
        }
        ok(dm(&home).arg(name).output().unwrap());
    }
}

#[test]
fn purge_restores_grouped_data_before_removal_even_without_a_launch() {
    for name in ["data", "cache", "logs", "plugins", "backups"] {
        for retained in [false, true] {
            let temp = TempDir::new().unwrap();
            let home = temp.path().join("home");
            fs::create_dir_all(&home).unwrap();
            install_legacy_plugin(&home, name);
            grouped_data(&home, name);
            let sentinel = home.join(name).join("other/keep");
            fs::create_dir_all(sentinel.parent().unwrap()).unwrap();
            fs::write(&sentinel, "other host or plugin content").unwrap();
            if retained {
                // Simulate a prior uninstall that kept the grouped layout.
                let connection = rusqlite::Connection::open(home.join("store.sqlite3")).unwrap();
                connection
                    .execute("DELETE FROM installed_plugins WHERE name = ?1", [name])
                    .unwrap();
                connection
                    .execute("INSERT INTO retained_plugin_data(name) VALUES (?1)", [name])
                    .unwrap();
                fs::remove_dir_all(home.join("plugins").join(name)).unwrap();
            }
            ok(dm(&home)
                .args(["uninstall", name, "--purge", "--yes"])
                .output()
                .unwrap());
            assert!(sentinel.is_file());
            for kind in ["config", "data", "cache"] {
                assert!(!home.join(name).join(kind).exists(), "{name}/{kind}");
                assert!(!home.join(kind).join(name).exists(), "{kind}/{name}");
            }
            assert!(!PluginStore::new(&home).has_retained_data(name).unwrap());
        }
    }
}

#[test]
fn doctor_restores_grouped_data_instead_of_deleting_it_as_orphaned() {
    for name in ["data", "cache", "logs", "plugins", "backups"] {
        let temp = TempDir::new().unwrap();
        let home = temp.path().join("home");
        fs::create_dir_all(&home).unwrap();
        install_legacy_plugin(&home, name);
        grouped_data(&home, name);
        let store = PluginStore::new(&home);
        let report = store.doctor(false).unwrap();
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.contains("needs recovery"))
        );
        assert!(home.join(name).join("config/keep").is_file());
        store.doctor(true).unwrap();
        for kind in ["config", "data", "cache"] {
            assert_eq!(
                fs::read_to_string(home.join(kind).join(name).join("keep")).unwrap(),
                format!("{name}-{kind}")
            );
        }
    }
}

#[test]
fn restoration_preserves_conflicting_and_other_owned_directories() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    install_legacy_plugin(&home, "data");
    grouped_data(&home, "data");
    fs::create_dir_all(home.join("config/data")).unwrap();
    fs::write(home.join("config/data/current"), "current").unwrap();
    let output = dm(&home).arg("data").output().unwrap();
    assert!(!output.status.success());
    assert!(stderr(&output).contains("Both"));
    assert!(home.join("data/config/keep").is_file());
    assert!(home.join("data/cache/keep").is_file());
    fs::remove_file(home.join("config/data/current")).unwrap();
    install_legacy_plugin(&home, "cache");
    ok(dm(&home).arg("data").output().unwrap());
    assert!(
        home.join("data/cache/keep").is_file(),
        "cache plugin's legacy data must stay"
    );
}

#[test]
fn completion_reads_grouped_reserved_data_without_migrating_it() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    install_legacy_plugin(&home, "logs");
    grouped_data(&home, "logs");
    let text = format!("{}completion = true\n", manifest("logs"));
    fs::write(home.join("plugins/logs/dm-plugin.toml"), &text).unwrap();
    rusqlite::Connection::open(home.join("store.sqlite3"))
        .unwrap()
        .execute(
            "UPDATE installed_plugins SET manifest = ?1 WHERE name = 'logs'",
            [&text],
        )
        .unwrap();
    fs::create_dir_all(home.join("data/logs")).unwrap();
    let output = PluginStore::new(&home)
        .complete_plugin("logs", &[])
        .unwrap();
    assert!(
        output.contains(
            &fs::canonicalize(home.join("logs/data"))
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned()
        ),
        "{output:?}"
    );
    assert!(home.join("logs/data/keep").is_file());
    assert!(!home.join("data/logs/keep").exists());
}

#[test]
fn recovery_does_not_move_an_orphan_package_into_reserved_plugin_data() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    install_legacy_plugin(&home, "plugins");
    install_legacy_plugin(&home, "data");
    rusqlite::Connection::open(home.join("store.sqlite3"))
        .unwrap()
        .execute("DELETE FROM installed_plugins WHERE name = 'data'", [])
        .unwrap();
    let store = PluginStore::new(&home);
    store.doctor(true).unwrap();
    store.info("data").unwrap();
    assert!(home.join("plugins/data/dm-plugin.toml").is_file());
    assert!(!home.join("data/plugins").exists());
}
