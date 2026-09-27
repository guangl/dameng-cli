//! Lifecycle hooks and the rollback they trigger when one fails.

use crate::common::*;
use dameng_cli::PluginStore;
use std::fs;
use tempfile::TempDir;

#[cfg(unix)]
#[test]
fn lifecycle_hooks_run_in_order() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    for (name, marker) in [
        ("pre.sh", "pre"),
        ("post.sh", "post"),
        ("preun.sh", "preun"),
        ("postun.sh", "postun"),
    ] {
        let path = source.join(name);
        fs::write(
            &path,
            format!(
                "#!/bin/sh\ntest \"$PWD\" = \"$DM_PLUGIN_DIR\" || exit 42\nprintf '{marker}\\n' >> \"$DM_PLUGIN_HOME/hooks.log\"\n"
            ),
        )
        .unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    fs::write(
        source.join("dm-plugin.toml"),
        r#"name = "probe"
version = "0.1.0"
description = "test"
api_version = 1
[hooks]
pre_install = "pre.sh"
post_install = "post.sh"
pre_uninstall = "preun.sh"
post_uninstall = "postun.sh"
"#,
    )
    .unwrap();

    let store = PluginStore::new(&home);
    store.install(source.to_str().unwrap()).unwrap();
    let log = home.join("hooks.log");
    let after_install = fs::read_to_string(&log).unwrap();
    assert!(after_install.contains("pre"), "{after_install}");
    assert!(after_install.contains("post"), "{after_install}");

    store.uninstall("probe").unwrap();
    let after_uninstall = fs::read_to_string(&log).unwrap();
    assert!(after_uninstall.contains("preun"), "{after_uninstall}");
    assert!(after_uninstall.contains("postun"), "{after_uninstall}");
}
#[test]
fn uninstall_restores_plugin_when_post_uninstall_hook_fails() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    let store = PluginStore::new(&home);
    store.install(source.to_str().unwrap()).unwrap();

    let installed = home.join("plugins/probe");
    fs::write(
        installed.join("dm-plugin.toml"),
        format!(
            "{}\n[hooks]\npost_uninstall = \"postun.sh\"\n",
            manifest("probe")
        ),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::write(installed.join("postun.sh"), "#!/bin/sh\nexit 1\n").unwrap();
        fs::set_permissions(
            installed.join("postun.sh"),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
    }

    let error = store.uninstall("probe").unwrap_err();
    assert!(error.to_string().contains("post-uninstall"), "{error:#}");
    assert_eq!(store.info("probe").unwrap().manifest.name, "probe");
}
#[cfg(unix)]
#[test]
fn update_rolls_back_when_post_install_hook_fails() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    let store = PluginStore::new(&home);
    store.install(source.to_str().unwrap()).unwrap();

    fs::write(
        source.join("dm-plugin.toml"),
        format!(
            "{}\n[hooks]\npost_install = \"post.sh\"\n",
            manifest("probe")
        ),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::write(source.join("post.sh"), "#!/bin/sh\nexit 1\n").unwrap();
        fs::set_permissions(source.join("post.sh"), fs::Permissions::from_mode(0o755)).unwrap();
    }

    let error = store.update("probe").unwrap_err();
    assert!(error.to_string().contains("post-install"), "{error:#}");
    assert_eq!(store.info("probe").unwrap().manifest.version, "0.1.0");
}
