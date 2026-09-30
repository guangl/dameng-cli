use crate::common::*;
use dameng_cli::PluginStore;
use std::fs;
use tempfile::TempDir;

#[test]
fn failed_purge_remains_retryable_for_installed_and_retained_plugins() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    for retained in [false, true] {
        let home = temp
            .path()
            .join(if retained { "retained" } else { "installed" });
        let store = PluginStore::new(&home);
        store.install(source.to_str().unwrap()).unwrap();
        if retained {
            ok(dm(&home).args(["uninstall", "probe"]).output().unwrap());
        }
        // A regular file at a directory path fails consistently, even as root.
        let blocked = home.join("backups/probe");
        fs::create_dir_all(blocked.parent().unwrap()).unwrap();
        fs::write(&blocked, "do not delete an unexpected file").unwrap();
        let failure = dm(&home)
            .args(["uninstall", "probe", "--purge", "--yes"])
            .output()
            .unwrap();
        assert!(!failure.status.success());
        assert!(stderr(&failure).contains("dm uninstall probe --purge --yes"));
        assert!(stderr(&failure).contains(blocked.to_str().unwrap()));
        assert!(store.info("probe").is_err());
        assert!(store.has_retained_data("probe").unwrap());
        // Repair must not reinterpret pending cleanup as abandoned data.
        let saved = home.join("data/probe/saved");
        fs::create_dir_all(saved.parent().unwrap()).unwrap();
        fs::write(&saved, "connection").unwrap();
        store.doctor(true).unwrap();
        assert!(saved.is_file());
        fs::remove_file(&blocked).unwrap();
        fs::create_dir(&blocked).unwrap();
        fs::write(blocked.join("backup"), "old binary").unwrap();
        ok(dm(&home)
            .args(["uninstall", "probe", "--purge", "--yes"])
            .output()
            .unwrap());
        assert!(!store.has_retained_data("probe").unwrap());
        for path in store.removal_paths("probe") {
            assert!(!path.exists(), "{}", path.display());
        }
    }
}
