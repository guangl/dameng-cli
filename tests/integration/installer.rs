//! The shell installers shipped in `scripts/`.

use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

/// Run `scripts/install-local.sh` against a throwaway install and data directory.
#[cfg(unix)]
fn run_local_installer(install_dir: &Path, home: &Path) -> Output {
    Command::new("sh")
        .arg("scripts/install-local.sh")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("DM_INSTALL_DIR", install_dir)
        .env("DM_PLUGIN_HOME", home)
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .unwrap()
}

#[cfg(unix)]
fn assert_installer_succeeded(output: &Output) {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(unix)]
#[test]
fn local_installer_installs_host_and_ssh_plugin() {
    let temp = TempDir::new().unwrap();
    let install_dir = temp.path().join("bin");
    let home = temp.path().join("home");
    let output = run_local_installer(&install_dir, &home);
    assert_installer_succeeded(&output);
    // Cargo's own progress output would drown out the installer summary.
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("Finished `release` profile"), "{stdout}");
    assert!(!stdout.contains("Compiling"), "{stdout}");
    assert!(install_dir.join("dm").is_file());
    let mut command = Command::new(install_dir.join("dm"));
    command.env("DM_PLUGIN_HOME", &home);
    let info = command.arg("info").arg("ssh").output().unwrap();
    assert!(
        info.status.success(),
        "{}",
        String::from_utf8_lossy(&info.stderr)
    );
    assert!(String::from_utf8_lossy(&info.stdout).contains("ssh"));
}

/// A store written by an older host keeps manifests the current host refuses to
/// parse (the removed `permissions` field). `dm info` fails for such a plugin, so
/// a check based on it would report the plugin as already installed instead of
/// upgrading it; the installer has to repair the metadata either way.
#[cfg(unix)]
#[test]
fn local_installer_repairs_a_legacy_manifest_in_the_store() {
    let temp = TempDir::new().unwrap();
    let install_dir = temp.path().join("bin");
    let home = temp.path().join("home");
    assert_installer_succeeded(&run_local_installer(&install_dir, &home));

    let legacy = concat!(
        "name = \"ssh\"\nversion = \"0.1.0\"\n",
        "description = \"Manage saved SSH server connections\"\n",
        "api_version = 1\nmin_host_version = \"0.2.0\"\nenvironment = []\n",
        "permissions = [\"filesystem\"]\n\n[hooks]\n",
    );
    let store = rusqlite::Connection::open(home.join("store.sqlite3")).unwrap();
    store
        .execute(
            "UPDATE installed_plugins SET manifest = ?1 WHERE name = 'ssh'",
            [legacy],
        )
        .unwrap();
    drop(store);

    let dm = install_dir.join("dm");
    let info = |args: &[&str]| {
        Command::new(&dm)
            .env("DM_PLUGIN_HOME", &home)
            .args(args)
            .output()
            .unwrap()
    };
    assert!(
        !info(&["info", "ssh"]).status.success(),
        "the legacy manifest has to be rejected"
    );

    assert_installer_succeeded(&run_local_installer(&install_dir, &home));
    let repaired = info(&["info", "ssh"]);
    assert!(
        repaired.status.success(),
        "{}",
        String::from_utf8_lossy(&repaired.stderr)
    );
}

#[cfg(unix)]
#[test]
fn installer_scripts_have_valid_shell_syntax() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for script in ["scripts/install.sh", "scripts/install-local.sh"] {
        let output = Command::new("sh")
            .args(["-n", root.join(script).to_str().unwrap()])
            .output()
            .unwrap();
        assert!(output.status.success(), "{script} has invalid shell syntax");
    }
}
