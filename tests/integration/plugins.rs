use dameng_cli::{Manifest, PluginStore};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};
use tempfile::TempDir;

fn dm(home: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_dm"));
    command
        .env("DM_PLUGIN_HOME", home)
        .env("CARGO_NET_OFFLINE", "true");
    command
}
fn ok(output: Output) -> String {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}
fn manifest(name: &str) -> String {
    format!("name = {name:?}\nversion = \"0.1.0\"\ndescription = \"test\"\napi_version = 1\n")
}
fn fixture(root: &Path) -> PathBuf {
    let source = root.join("Rust plugin source");
    fs::create_dir_all(source.join("src")).unwrap();
    fs::write(source.join("dm-plugin.toml"), manifest("probe")).unwrap();
    let sdk = Path::new(env!("CARGO_MANIFEST_DIR")).join("crates/dm-plugin-sdk");
    fs::write(
        source.join("Cargo.toml"),
        format!(
            r#"
[package]
name = "dm-plugin-probe"
version = "0.1.0"
edition = "2024"
[[bin]]
name = "dm-probe"
path = "src/main.rs"
[dependencies]
dm-plugin-sdk = {{ path = {:?} }}
"#,
            sdk.to_str().unwrap()
        ),
    )
    .unwrap();
    fs::write(
        source.join("src/main.rs"),
        r#"
use dm_plugin_sdk::{Context, Plugin, PluginResult};
use std::io::{self, Read};
struct Probe;
impl Plugin for Probe {
    fn run(&self, context: Context) -> PluginResult {
        println!("cwd={}", std::env::current_dir()?.display());
        println!("home={}", context.home.display());
        println!("legacy_home={}", std::env::var("DM_HOME").unwrap_or_else(|_| "missing".into()));
        println!("plugin={}", context.plugin_dir.display());
        println!("config={}", context.config_dir.display());
        println!("data={}", context.data_dir.display());
        println!("cache={}", context.cache_dir.display());
        println!("capabilities={}", context.capabilities.join(","));
        println!("secret={}", std::env::var("DM_TEST_SECRET").unwrap_or_else(|_| "filtered".into()));
        for arg in &context.args { println!("arg={}", arg.to_string_lossy()); }
        eprintln!("plugin stderr");
        let mut input = String::new();
        io::stdin().read_to_string(&mut input)?;
        print!("stdin={input}");
        Ok(if context.args.iter().any(|a| a == "--fail") { 23 } else { 0 })
    }
}
fn main() { dm_plugin_sdk::run(Probe); }
"#,
    )
    .unwrap();
    ok(Command::new("cargo")
        .args(["generate-lockfile", "--offline", "--manifest-path"])
        .arg(source.join("Cargo.toml"))
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .unwrap());
    ok(Command::new("cargo")
        .args([
            "build",
            "--release",
            "--locked",
            "--offline",
            "--manifest-path",
        ])
        .arg(source.join("Cargo.toml"))
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .unwrap());
    let executable = format!("dm-probe{}", std::env::consts::EXE_SUFFIX);
    fs::copy(
        source.join("target/release").join(&executable),
        source.join(&executable),
    )
    .unwrap();
    source
}

#[test]
fn rust_plugin_lifecycle_and_process_contract() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("dm home");
    let source = fixture(temp.path());
    assert!(ok(dm(&home).arg("list").output().unwrap()).contains("No plugins installed"));
    assert!(home.join("store.sqlite3").is_file());
    assert!(
        ok(dm(&home).arg("install").arg(&source).output().unwrap()).contains("Installed probe")
    );
    let listed = ok(dm(&home).arg("list").output().unwrap());
    assert!(listed.contains("probe") && listed.contains("0.1.0"));
    assert_eq!(
        &fs::read(home.join("store.sqlite3")).unwrap()[..16],
        b"SQLite format 3\0"
    );
    assert!(
        !dm(&home)
            .arg("install")
            .arg(&source)
            .output()
            .unwrap()
            .status
            .success()
    );
    // Runtime must be independent of the source tree and build products.
    fs::remove_dir_all(&source).unwrap();
    let mut child = dm(&home)
        .args(["probe", "hello world", "--help", "--", "--fail"])
        .env("DM_TEST_SECRET", "must-not-leak")
        .current_dir(temp.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"input payload\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(23));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("arg=hello world\narg=--help\narg=--\narg=--fail\n"),
        "{stdout}"
    );
    assert!(stdout.contains("stdin=input payload"));
    let cwd = stdout
        .lines()
        .find_map(|line| line.strip_prefix("cwd="))
        .unwrap();
    assert_eq!(
        fs::canonicalize(cwd).unwrap(),
        fs::canonicalize(temp.path()).unwrap()
    );
    assert!(stdout.contains(&format!(
        "home={}",
        fs::canonicalize(&home).unwrap().display()
    )));
    assert!(stdout.contains(&format!(
        "legacy_home={}",
        fs::canonicalize(&home).unwrap().display()
    )));
    assert!(stdout.contains(
        &format!("plugin={}", fs::canonicalize(home.join("plugins/probe")).unwrap().display())
    ));
    for directory in ["config", "data", "cache"] {
        assert!(stdout.contains(&format!(
            "{directory}={}",
            fs::canonicalize(home.join(directory).join("probe"))
                .unwrap()
                .display()
        )));
    }
    assert!(stdout.contains("capabilities=config-dirs-v1"));
    assert!(stdout.contains("secret=filtered"));
    assert!(String::from_utf8_lossy(&output.stderr).contains("plugin stderr"));
    // A broken plugin must remain removable.
    fs::write(home.join("plugins/probe/dm-plugin.toml"), "broken").unwrap();
    ok(dm(&home).args(["uninstall", "probe"]).output().unwrap());
    assert!(ok(dm(&home).arg("list").output().unwrap()).contains("No plugins installed"));
    for directory in ["config", "data", "cache"] {
        assert!(!home.join(directory).join("probe").exists());
    }
    assert!(!dm(&home).arg("probe").output().unwrap().status.success());
}

#[test]
fn source_without_prebuilt_binary_is_rejected() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    fs::remove_file(source.join(format!("dm-probe{}", std::env::consts::EXE_SUFFIX))).unwrap();
    let home = temp.path().join("home");
    let output = dm(&home).arg("install").arg(&source).output().unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("prebuilt"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read_dir(home.join("plugins")).unwrap().count(), 0);
}

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

#[test]
fn doctor_repairs_missing_checksums_and_stale_transactions() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    let store = PluginStore::new(&home);
    store.install(source.to_str().unwrap()).unwrap();
    rusqlite::Connection::open(home.join("store.sqlite3"))
        .unwrap()
        .execute(
            "UPDATE installed_plugins SET checksum = '' WHERE name = 'probe'",
            [],
        )
        .unwrap();
    let stale = home.join("plugins/.install-stale");
    fs::create_dir(&stale).unwrap();

    let report = store.doctor(false).unwrap();
    assert_eq!(report.issues.len(), 2);
    let repaired = store.doctor(true).unwrap();
    assert_eq!(repaired.repairs.len(), 2);
    assert!(!stale.exists());
    store.verify(Some("probe")).unwrap();
}

#[test]
fn doctor_restores_an_interrupted_removal() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    let store = PluginStore::new(&home);
    store.install(source.to_str().unwrap()).unwrap();
    let interrupted = home.join("plugins/.remove-interrupted");
    fs::create_dir(&interrupted).unwrap();
    fs::rename(home.join("plugins/probe"), interrupted.join("package")).unwrap();

    let report = store.doctor(true).unwrap();
    assert!(
        report
            .repairs
            .iter()
            .any(|repair| repair.contains("restored interrupted"))
    );
    store.verify(Some("probe")).unwrap();
}

#[test]
fn doctor_cleans_orphaned_per_plugin_directories() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let store = PluginStore::new(&home);
    for directory in ["config", "data", "cache"] {
        let orphan = home.join(directory).join("orphan");
        fs::create_dir_all(&orphan).unwrap();
        fs::write(orphan.join("leftover"), "leftover").unwrap();
    }

    let report = store.doctor(false).unwrap();
    assert_eq!(report.issues.len(), 3);
    let repaired = store.doctor(true).unwrap();
    assert!(
        repaired
            .repairs
            .iter()
            .any(|repair| repair.contains("config/orphan"))
    );
    for directory in ["config", "data", "cache"] {
        assert!(!home.join(directory).join("orphan").exists());
    }
}

#[test]
fn json_cli_reports_manifest_name() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    ok(dm(&home)
        .args(["install", source.to_str().unwrap()])
        .output()
        .unwrap());
    let output = ok(dm(&home).args(["list", "--json"]).output().unwrap());
    let json: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(json[0]["manifest"]["name"], "probe");
    assert!(ok(dm(&home).args(["completions", "bash"]).output().unwrap()).contains("_dm"));
}

#[cfg(unix)]
#[test]
fn unwritable_dm_home_reports_actionable_error() {
    use std::os::unix::fs::PermissionsExt;

    let temp = TempDir::new().unwrap();
    let home = temp.path().join("readonly");
    fs::create_dir_all(&home).unwrap();
    fs::set_permissions(&home, fs::Permissions::from_mode(0o555)).unwrap();

    let error = PluginStore::new(&home).list().unwrap_err();
    assert!(error.to_string().contains("is not writable"), "{error:#}");

    fs::set_permissions(&home, fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn home_must_not_be_inside_plugin_source() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let store = PluginStore::new(source.join("home"));
    assert!(
        store
            .install(source.to_str().unwrap())
            .unwrap_err()
            .to_string()
            .contains("must not contain")
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

#[test]
fn concurrent_install_publishes_one_complete_plugin() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    let first = dm(&home)
        .arg("install")
        .arg(&source)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let second = dm(&home)
        .arg("install")
        .arg(&source)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let results = [
        first.wait_with_output().unwrap(),
        second.wait_with_output().unwrap(),
    ];
    assert_eq!(results.iter().filter(|r| r.status.success()).count(), 1);
    let listed = ok(dm(&home).arg("list").output().unwrap());
    assert!(listed.contains("probe") && listed.contains("0.1.0"));
    assert_eq!(fs::read_dir(home.join("plugins")).unwrap().count(), 1);
    let binary = home
        .join("plugins/probe")
        .join(format!("dm-probe{}", std::env::consts::EXE_SUFFIX));
    let output = Command::new(&binary)
        .env_remove("DM_PLUGIN_API_VERSION")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Run this plugin through dm"));
    let output = Command::new(binary)
        .env("DM_PLUGIN_API_VERSION", "999")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Unsupported host plugin API"));
}

#[cfg(unix)]
#[test]
fn prebuilt_release_is_used_before_source_build() {
    use std::os::unix::fs::PermissionsExt;

    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    fs::remove_file(source.join(format!("dm-probe{}", std::env::consts::EXE_SUFFIX))).unwrap();
    let home = temp.path().join("home");
    let tools = temp.path().join("tools");
    fs::create_dir(&tools).unwrap();

    let git = tools.join("git");
    fs::write(
        &git,
        "#!/bin/sh\nfor destination do :; done\ncase \" $* \" in *\" clone \"*) cp -R \"$FAKE_GIT_SOURCE\" \"$destination\" ;; *\" rev-parse \"*) printf '%040d\\n' 1 ;; esac\n",
    )
    .unwrap();
    fs::set_permissions(&git, fs::Permissions::from_mode(0o755)).unwrap();

    let fake_bin = temp.path().join("fake-bin");
    fs::write(&fake_bin, "prebuilt-binary").unwrap();
    let curl = tools.join("curl");
    fs::write(
        &curl,
        "#!/bin/sh\nout=\nurl=\nwhile [ \"$#\" -gt 0 ]; do\n  case \"$1\" in\n    --output) out=\"$2\"; shift 2;;\n    *) url=\"$1\"; shift;;\n  esac\ndone\ncase \"$url\" in\n  *.sha256) exit 22 ;;\n  *) cp \"$FAKE_BIN\" \"$out\" ;;\nesac\n",
    )
    .unwrap();
    fs::set_permissions(&curl, fs::Permissions::from_mode(0o755)).unwrap();

    let path = std::env::join_paths(
        std::iter::once(tools.clone())
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let mut command = dm(&home);
    command
        .env("PATH", &path)
        .env("FAKE_GIT_SOURCE", &source)
        .env("FAKE_BIN", &fake_bin);
    let output = command
        .args(["install", "https://github.com/example/probe.git"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("no SHA-256 sidecar"), "{stderr}");
    assert!(
        !stderr.contains("Building probe 0.1.0 from source"),
        "{stderr}"
    );
    let installed = home
        .join("plugins/probe")
        .join(format!("dm-probe{}", std::env::consts::EXE_SUFFIX));
    assert_eq!(fs::read(installed).unwrap(), b"prebuilt-binary");
}

#[test]
fn outdated_command_reports_newer_local_version() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    ok(dm(&home)
        .args(["install", source.to_str().unwrap()])
        .output()
        .unwrap());
    assert!(ok(dm(&home).args(["outdated"]).output().unwrap()).contains("current"));

    fs::write(
        source.join("dm-plugin.toml"),
        manifest("probe").replace("0.1.0", "0.2.0"),
    )
    .unwrap();
    let output = ok(dm(&home).args(["outdated"]).output().unwrap());
    assert!(output.contains("0.2.0"));
    assert!(output.contains("update available"));
    let json = ok(dm(&home).args(["outdated", "--json"]).output().unwrap());
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed[0]["update_available"], true);
}

#[test]
fn update_all_command_updates_installed_plugins() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    ok(dm(&home)
        .args(["install", source.to_str().unwrap()])
        .output()
        .unwrap());
    let output = ok(dm(&home).args(["update", "--all"]).output().unwrap());
    assert!(output.contains("Updated probe to 0.1.0"), "{output}");
    assert!(home.join("plugins/probe/dm-plugin.toml").is_file());
}

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

#[test]
fn cli_reporting_branches_cover_info_verify_update_doctor() {
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
    ok(dm(&home)
        .args(["install", source.to_str().unwrap()])
        .output()
        .unwrap());

    let info = ok(dm(&home).args(["info", "probe"]).output().unwrap());
    assert!(info.contains("Name: probe"), "{info}");
    assert!(info.contains("Version: 0.1.0"), "{info}");
    assert!(info.contains("Permissions: network"), "{info}");
    assert!(info.contains("Environment: DM_DATABASE_URL"), "{info}");
    let info_json = ok(dm(&home)
        .args(["info", "--json", "probe"])
        .output()
        .unwrap());
    let parsed: serde_json::Value = serde_json::from_str(&info_json).unwrap();
    assert_eq!(parsed["manifest"]["name"], "probe");

    let verify = ok(dm(&home).args(["verify", "probe"]).output().unwrap());
    assert!(verify.contains("Verified probe"), "{verify}");

    ok(dm(&home).args(["update", "probe"]).output().unwrap());

    fs::write(
        source.join("dm-plugin.toml"),
        format!(
            r#"{}permissions = ["network"]
environment = ["DM_DATABASE_URL"]
"#,
            manifest("probe").replace("0.1.0", "0.2.0")
        ),
    )
    .unwrap();
    let cargo = fs::read_to_string(source.join("Cargo.toml")).unwrap();
    fs::write(source.join("Cargo.toml"), cargo.replace("0.1.0", "0.2.0")).unwrap();
    ok(Command::new("cargo")
        .args(["generate-lockfile", "--offline", "--manifest-path"])
        .arg(source.join("Cargo.toml"))
        .output()
        .unwrap());
    ok(dm(&home).args(["update", "probe"]).output().unwrap());

    let healthy = ok(dm(&home).args(["doctor"]).output().unwrap());
    assert!(healthy.contains("Plugin store is healthy"), "{healthy}");
    let doctor_json = ok(dm(&home).args(["doctor", "--json"]).output().unwrap());
    let doctor: serde_json::Value = serde_json::from_str(&doctor_json).unwrap();
    assert!(doctor["issues"].is_array());

    let orphan = home.join("config").join("orphan");
    fs::create_dir_all(&orphan).unwrap();
    fs::write(orphan.join("leftover"), "leftover").unwrap();
    let issues = ok(dm(&home).args(["doctor"]).output().unwrap());
    assert!(issues.contains("Issue:"), "{issues}");
    assert!(issues.contains("Run dm doctor --repair"), "{issues}");
    let repaired = ok(dm(&home).args(["doctor", "--repair"]).output().unwrap());
    assert!(repaired.contains("Repaired:"), "{repaired}");
    assert!(!orphan.exists());

    fs::remove_file(source.join(format!("dm-probe{}", std::env::consts::EXE_SUFFIX))).unwrap();
    let failed = dm(&home).args(["update", "--all"]).output().unwrap();
    assert!(!failed.status.success());
    assert!(
        String::from_utf8_lossy(&failed.stderr).contains("Some updates failed"),
        "{}",
        String::from_utf8_lossy(&failed.stderr)
    );
}

#[test]
fn relative_dm_home_is_resolved_against_current_dir() {
    let temp = TempDir::new().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_dm"))
        .current_dir(temp.path())
        .env("DM_PLUGIN_HOME", "relhome")
        .args(["list"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(temp.path().join("relhome/store.sqlite3").is_file());
}

#[test]
fn sqlite_store_open_error_is_actionable() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    fs::create_dir_all(home.join("store.sqlite3")).unwrap();
    let output = dm(&home).args(["list"]).output().unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("Cannot open SQLite plugin store"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(unix)]
#[test]
fn doctor_reports_and_repairs_database_and_disk_mismatches() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    let store = PluginStore::new(&home);
    store.install(source.to_str().unwrap()).unwrap();

    fs::remove_dir_all(home.join("plugins/probe")).unwrap();
    let report = store.doctor(false).unwrap();
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.contains("database entry without plugin directory")),
        "{report:?}"
    );
    store.doctor(true).unwrap();
    assert!(store.list().unwrap().is_empty());

    let disk = home.join("plugins/disk");
    fs::create_dir_all(&disk).unwrap();
    fs::write(disk.join("dm-plugin.toml"), manifest("disk")).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::write(disk.join("dm-disk"), "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(disk.join("dm-disk"), fs::Permissions::from_mode(0o755)).unwrap();
    }
    let report = store.doctor(false).unwrap();
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.contains("plugin directory without database entry")),
        "{report:?}"
    );
    store.doctor(true).unwrap();
    assert_eq!(store.info("disk").unwrap().manifest.name, "disk");
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

#[cfg(unix)]
#[test]
fn checkout_revision_failure_is_reported() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    let tools = temp.path().join("tools");
    fs::create_dir(&tools).unwrap();

    let git = tools.join("git");
    fs::write(
        &git,
        r#"#!/bin/sh
last=""
for arg do last="$arg"; done
case " $* " in
  *" checkout "*) exit 1;;
  *" clone "*) cp -R "$FAKE_GIT_SOURCE" "$last";;
  *" rev-parse "*) printf '%040d
' 1;;
esac
"#,
    )
    .unwrap();
    fs::set_permissions(&git, fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::join_paths(
        std::iter::once(tools).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();

    let output = dm(&home)
        .env("PATH", &path)
        .env("FAKE_GIT_SOURCE", &source)
        .args([
            "install",
            "https://example.invalid/probe.git",
            "--rev",
            "bad",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("could not be checked out"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn install_reports_malformed_store_query_error() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    rusqlite::Connection::open(home.join("store.sqlite3"))
        .unwrap()
        .execute_batch("CREATE TABLE installed_plugins (manifest TEXT NOT NULL)")
        .unwrap();

    let output = dm(&home).arg("install").arg(&source).output().unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("no such column"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn install_reports_sqlite_insert_failure() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    rusqlite::Connection::open(home.join("store.sqlite3"))
        .unwrap()
        .execute_batch(
            "CREATE TABLE installed_plugins (
                 name TEXT PRIMARY KEY,
                 manifest TEXT NOT NULL,
                 installed_at INTEGER NOT NULL DEFAULT (unixepoch()),
                 source TEXT,
                 revision TEXT,
                 source_ref TEXT,
                 checksum TEXT NOT NULL DEFAULT '',
                 extra TEXT NOT NULL
             ) STRICT;",
        )
        .unwrap();

    let output = dm(&home).arg("install").arg(&source).output().unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("Record installed plugin in SQLite"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(unix)]
#[test]
fn update_https_source_checks_out_and_updates() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    let store = PluginStore::new(&home);
    store.install(source.to_str().unwrap()).unwrap();
    rusqlite::Connection::open(home.join("store.sqlite3"))
        .unwrap()
        .execute(
            "UPDATE installed_plugins SET source = ?1 WHERE name = 'probe'",
            ["https://github.com/example/probe.git"],
        )
        .unwrap();

    let tools = temp.path().join("tools");
    fs::create_dir(&tools).unwrap();
    let git = tools.join("git");
    fs::write(
        &git,
        r#"#!/bin/sh
last=""
for arg do last="$arg"; done
case " $* " in
  *" clone "*) cp -R "$FAKE_GIT_SOURCE" "$last";;
  *" rev-parse "*) printf '%040d
' 1;;
esac
"#,
    )
    .unwrap();
    fs::set_permissions(&git, fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::join_paths(
        std::iter::once(tools).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();

    let output = dm(&home)
        .env("PATH", &path)
        .env("FAKE_GIT_SOURCE", &source)
        .args(["update", "probe"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("Updated probe"));
}

#[test]
fn outdated_without_source_reports_no_available_version() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    let store = PluginStore::new(&home);
    store.install(source.to_str().unwrap()).unwrap();
    rusqlite::Connection::open(home.join("store.sqlite3"))
        .unwrap()
        .execute(
            "UPDATE installed_plugins SET source = NULL WHERE name = 'probe'",
            [],
        )
        .unwrap();

    let statuses = store.outdated().unwrap();
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].available_version, None);
    assert!(!statuses[0].update_available);
}

#[cfg(unix)]
#[test]
fn outdated_https_source_uses_fake_git() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    let store = PluginStore::new(&home);
    store.install(source.to_str().unwrap()).unwrap();
    rusqlite::Connection::open(home.join("store.sqlite3"))
        .unwrap()
        .execute(
            "UPDATE installed_plugins SET source = ?1 WHERE name = 'probe'",
            ["https://github.com/example/probe.git"],
        )
        .unwrap();

    let tools = temp.path().join("tools");
    fs::create_dir(&tools).unwrap();
    let git = tools.join("git");
    fs::write(
        &git,
        r#"#!/bin/sh
last=""
for arg do last="$arg"; done
case " $* " in
  *" clone "*) cp -R "$FAKE_GIT_SOURCE" "$last";;
  *" rev-parse "*) printf '%040d
' 1;;
esac
"#,
    )
    .unwrap();
    fs::set_permissions(&git, fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::join_paths(
        std::iter::once(tools).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();

    let output = dm(&home)
        .env("PATH", &path)
        .env("FAKE_GIT_SOURCE", &source)
        .args(["outdated"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("0.1.0"));
}

#[test]
fn doctor_reports_invalid_plugin() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    let store = PluginStore::new(&home);
    store.install(source.to_str().unwrap()).unwrap();

    rusqlite::Connection::open(home.join("store.sqlite3"))
        .unwrap()
        .execute(
            "INSERT INTO installed_plugins (name, manifest, checksum) VALUES (?1, ?2, '')",
            rusqlite::params!["bad", manifest("bad")],
        )
        .unwrap();
    fs::create_dir_all(home.join("plugins/bad")).unwrap();
    fs::write(home.join("plugins/bad/dm-plugin.toml"), "invalid").unwrap();

    let report = store.doctor(false).unwrap();
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.contains("invalid plugin bad")),
        "{report:?}"
    );
}

#[test]
fn doctor_reports_read_dir_error() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let store = PluginStore::new(&home);
    fs::create_dir_all(&home).unwrap();
    fs::write(home.join("config"), "not a directory").unwrap();

    let error = store.doctor(false).unwrap_err();
    assert!(error.to_string().contains("Read"), "{error:#}");
}

#[test]
fn doctor_skips_files_and_installed_names() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    let store = PluginStore::new(&home);
    store.install(source.to_str().unwrap()).unwrap();

    fs::create_dir_all(home.join("config")).unwrap();
    fs::write(home.join("config/somefile"), "file").unwrap();
    fs::create_dir_all(home.join("config/probe")).unwrap();

    let report = store.doctor(false).unwrap();
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.contains("orphaned config")),
        "{report:?}"
    );
}

#[test]
fn checkout_rejects_whitespace_revision() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let output = dm(&home)
        .args([
            "install",
            "https://example.invalid/probe.git",
            "--rev",
            "bad rev",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("revision"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(unix)]
#[test]
fn prebuilt_checksum_mismatch_is_rejected() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    fs::remove_file(source.join(format!("dm-probe{}", std::env::consts::EXE_SUFFIX))).unwrap();
    let home = temp.path().join("home");
    let tools = temp.path().join("tools");
    fs::create_dir(&tools).unwrap();

    let git = tools.join("git");
    fs::write(
        &git,
        r#"#!/bin/sh
last=""
for arg do last="$arg"; done
case " $* " in
  *" clone "*) cp -R "$FAKE_GIT_SOURCE" "$last";;
  *" rev-parse "*) printf '%040d
' 1;;
esac
"#,
    )
    .unwrap();
    fs::set_permissions(&git, fs::Permissions::from_mode(0o755)).unwrap();

    let fake_bin = temp.path().join("fake-bin");
    fs::write(&fake_bin, "prebuilt-binary").unwrap();
    let curl = tools.join("curl");
    fs::write(
        &curl,
        r#"#!/bin/sh
out=""
url=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --output) out="$2"; shift 2;;
    *) url="$1"; shift;;
  esac
done
case "$url" in
  *.sha256) printf '0000000000000000000000000000000000000000000000000000000000000000' > "$out";;
  *) cp "$FAKE_BIN" "$out";;
esac
"#,
    )
    .unwrap();
    fs::set_permissions(&curl, fs::Permissions::from_mode(0o755)).unwrap();

    let path = std::env::join_paths(
        std::iter::once(tools).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let output = dm(&home)
        .env("PATH", &path)
        .env("FAKE_GIT_SOURCE", &source)
        .env("FAKE_BIN", &fake_bin)
        .args(["install", "https://github.com/example/probe.git"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("SHA-256 mismatch"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(unix)]
#[test]
fn prebuilt_missing_release_is_rejected() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    fs::remove_file(source.join(format!("dm-probe{}", std::env::consts::EXE_SUFFIX))).unwrap();
    let home = temp.path().join("home");
    let tools = temp.path().join("tools");
    fs::create_dir(&tools).unwrap();

    let git = tools.join("git");
    fs::write(
        &git,
        r#"#!/bin/sh
last=""
for arg do last="$arg"; done
case " $* " in
  *" clone "*) cp -R "$FAKE_GIT_SOURCE" "$last";;
  *" rev-parse "*) printf '%040d
' 1;;
esac
"#,
    )
    .unwrap();
    fs::set_permissions(&git, fs::Permissions::from_mode(0o755)).unwrap();

    let curl = tools.join("curl");
    fs::write(&curl, "#!/bin/sh\nexit 1\n").unwrap();
    fs::set_permissions(&curl, fs::Permissions::from_mode(0o755)).unwrap();

    let path = std::env::join_paths(
        std::iter::once(tools).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let output = dm(&home)
        .env("PATH", &path)
        .env("FAKE_GIT_SOURCE", &source)
        .args(["install", "https://github.com/example/probe.git"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("No prebuilt plugin"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(unix)]
#[test]
fn prebuilt_checksum_match_is_accepted() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    fs::remove_file(source.join(format!("dm-probe{}", std::env::consts::EXE_SUFFIX))).unwrap();
    let home = temp.path().join("home");
    let tools = temp.path().join("tools");
    fs::create_dir(&tools).unwrap();

    let git = tools.join("git");
    fs::write(
        &git,
        r#"#!/bin/sh
last=""
for arg do last="$arg"; done
case " $* " in
  *" clone "*) cp -R "$FAKE_GIT_SOURCE" "$last";;
  *" rev-parse "*) printf '%040d
' 1;;
esac
"#,
    )
    .unwrap();
    fs::set_permissions(&git, fs::Permissions::from_mode(0o755)).unwrap();

    let fake_bin = temp.path().join("fake-bin");
    let bytes = b"prebuilt-binary";
    fs::write(&fake_bin, bytes).unwrap();
    let digest = format!("{:x}", Sha256::digest(bytes));
    let curl = tools.join("curl");
    fs::write(
        &curl,
        r#"#!/bin/sh
out=""
url=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --output) out="$2"; shift 2;;
    *) url="$1"; shift;;
  esac
done
case "$url" in
  *.sha256) printf '%s' "__DIGEST__" > "$out";;
  *) cp "$FAKE_BIN" "$out";;
esac
"#
        .replace("__DIGEST__", &digest),
    )
    .unwrap();
    fs::set_permissions(&curl, fs::Permissions::from_mode(0o755)).unwrap();

    let path = std::env::join_paths(
        std::iter::once(tools).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let output = dm(&home)
        .env("PATH", &path)
        .env("FAKE_GIT_SOURCE", &source)
        .env("FAKE_BIN", &fake_bin)
        .args(["install", "https://github.com/example/probe.git"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
