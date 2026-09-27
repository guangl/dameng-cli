//! `PluginStore` behavior without a full CLI round trip.

use crate::common::*;
use dameng_cli::Manifest;
use dameng_cli::PluginStore;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn manifest_rejects_invalid_names_versions_and_foreign_entrypoints() {
    let temp = TempDir::new().unwrap();
    for name in [
        "../escape",
        "install",
        "list",
        "help",
        "version",
        "uninstall",
        "doctor",
        "self-update",
        "Upper",
        "",
        "a/b",
        "con",
        "com1",
    ] {
        fs::write(temp.path().join("dm-plugin.toml"), manifest(name)).unwrap();
        assert!(Manifest::read(temp.path()).is_err(), "accepted {name}");
    }
    for text in [
        manifest("valid").replace("api_version = 1", "api_version = 2"),
        manifest("valid") + "executable = \"script.py\"\n",
        manifest("valid").replace("version = \"0.1.0\"", "version = \"\""),
    ] {
        fs::write(temp.path().join("dm-plugin.toml"), text).unwrap();
        assert!(Manifest::read(temp.path()).is_err());
    }
}
#[test]
fn local_prebuilt_package_does_not_require_cargo_manifest() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let store = PluginStore::new(temp.path().join("home"));
    fs::remove_file(source.join("Cargo.toml")).unwrap();
    store.install(source.to_str().unwrap()).unwrap();
    assert_eq!(store.info("probe").unwrap().manifest.name, "probe");
}
#[test]
fn traversal_and_unknown_sources_are_rejected() {
    let temp = TempDir::new().unwrap();
    let store = PluginStore::new(temp.path());
    assert!(store.uninstall("../outside").is_err());
    assert!(store.run("../outside", &[]).is_err());
    assert!(store.install("unknown").is_err());
    assert!(store.install("probe").is_err());
}
#[test]
fn plugin_metadata_verification_and_atomic_update() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    let store = PluginStore::new(&home);
    store.install(source.to_str().unwrap()).unwrap();

    let info = store.info("probe").unwrap();
    assert_eq!(info.manifest.version, "0.1.0");
    assert_eq!(info.checksum.len(), 64);
    assert_eq!(
        info.source.as_deref(),
        source.canonicalize().unwrap().to_str()
    );
    assert_eq!(store.verify(Some("probe")).unwrap(), ["probe"]);

    fs::write(
        source.join("dm-plugin.toml"),
        manifest("probe").replace("0.1.0", "0.2.0"),
    )
    .unwrap();
    let cargo = fs::read_to_string(source.join("Cargo.toml")).unwrap();
    fs::write(source.join("Cargo.toml"), cargo.replace("0.1.0", "0.2.0")).unwrap();
    ok(Command::new("cargo")
        .args(["generate-lockfile", "--offline", "--manifest-path"])
        .arg(source.join("Cargo.toml"))
        .output()
        .unwrap());
    assert_eq!(store.update("probe").unwrap().version, "0.2.0");
    let good_checksum = store.info("probe").unwrap().checksum;

    fs::remove_file(source.join(format!("dm-probe{}", std::env::consts::EXE_SUFFIX))).unwrap();
    assert!(store.update("probe").is_err());
    let current = store.info("probe").unwrap();
    assert_eq!(current.manifest.version, "0.2.0");
    assert_eq!(current.checksum, good_checksum);
    store.verify(Some("probe")).unwrap();
}
#[test]
fn permissions_are_recorded_without_consent_gate() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    fs::write(
        source.join("dm-plugin.toml"),
        format!(
            r#"{}permissions = ["network"]
environment = ["DM_DATABASE_URL"]
"#,
            manifest("probe")
        ),
    )
    .unwrap();
    let home = temp.path().join("home");
    let store = PluginStore::new(&home);

    store.install(source.to_str().unwrap()).unwrap();
    assert_eq!(
        store.info("probe").unwrap().manifest.permissions,
        vec!["network"]
    );

    fs::write(
        source.join("dm-plugin.toml"),
        format!(
            r#"{}permissions = ["network", "filesystem"]
environment = ["DM_DATABASE_URL"]
"#,
            manifest("probe")
        ),
    )
    .unwrap();
    store.update("probe").unwrap();
    let info = store.info("probe").unwrap();
    assert!(
        info.manifest
            .permissions
            .contains(&"filesystem".to_string())
    );
    assert_eq!(
        info.manifest.environment,
        vec!["DM_DATABASE_URL".to_string()]
    );
}
#[cfg(unix)]
#[test]
fn symlinked_installed_directory_is_not_executed_or_removed() {
    use std::os::unix::fs::symlink;
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let source = fixture(temp.path());
    let store = PluginStore::new(&home);
    store.install(source.to_str().unwrap()).unwrap();
    fs::remove_dir_all(home.join("plugins/probe")).unwrap();
    let outside = temp.path().join("outside");
    fs::create_dir(&outside).unwrap();
    symlink(&outside, home.join("plugins/probe")).unwrap();
    assert!(store.run("probe", &[]).is_err());
    assert!(store.uninstall("probe").is_err());
    assert!(outside.is_dir());
}
#[cfg(unix)]
#[test]
fn plugin_run_preserves_signal_exit_code() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("sig-source");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("dm-plugin.toml"), manifest("sig")).unwrap();
    fs::write(source.join("dm-sig"), "#!/bin/sh\nkill -TERM $$\n").unwrap();
    fs::set_permissions(source.join("dm-sig"), fs::Permissions::from_mode(0o755)).unwrap();

    let store = PluginStore::new(temp.path().join("home"));
    store.install(source.to_str().unwrap()).unwrap();
    assert_eq!(store.run("sig", &[]).unwrap(), 143);
}
