//! The shell installers shipped in `scripts/`.

use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

#[cfg(unix)]
#[test]
fn local_installer_installs_host_and_ssh_plugin() {
    let temp = TempDir::new().unwrap();
    let install_dir = temp.path().join("bin");
    let home = temp.path().join("home");
    let output = Command::new("sh")
        .arg("scripts/install-local.sh")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("DM_INSTALL_DIR", &install_dir)
        .env("DM_PLUGIN_HOME", &home)
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
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
