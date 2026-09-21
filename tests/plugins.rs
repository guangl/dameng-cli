use dameng_cli::{Manifest, PluginStore};
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
        .env("DM_HOME", home)
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
        .output()
        .unwrap());
    source
}

#[test]
fn rust_plugin_lifecycle_and_process_contract() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("dm home");
    let source = fixture(temp.path());
    assert!(ok(dm(&home).arg("list").output().unwrap()).is_empty());
    assert!(home.join("store.sqlite3").is_file());
    assert!(
        ok(dm(&home).arg("install").arg(&source).output().unwrap()).contains("Installed probe")
    );
    assert!(ok(dm(&home).arg("list").output().unwrap()).contains("probe\t0.1.0"));
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
    assert!(ok(dm(&home).arg("list").output().unwrap()).is_empty());
    for directory in ["config", "data", "cache"] {
        assert!(!home.join(directory).join("probe").exists());
    }
    assert!(!dm(&home).arg("probe").output().unwrap().status.success());
}

#[test]
fn failed_rust_build_leaves_no_partial_install() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    fs::write(source.join("src/main.rs"), "this is not Rust").unwrap();
    let home = temp.path().join("home");
    assert!(
        !dm(&home)
            .arg("install")
            .arg(source)
            .output()
            .unwrap()
            .status
            .success()
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
        "registry",
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
fn requires_rust_sdk_and_matching_binary_contract() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let original = fs::read_to_string(source.join("Cargo.toml")).unwrap();
    let store = PluginStore::new(temp.path().join("home"));
    for text in [
        original.replace("dm-plugin-sdk =", "other-sdk ="),
        original.replace("dm-probe", "wrong-bin"),
        original.replace("0.1.0", "0.2.0"),
    ] {
        fs::write(source.join("Cargo.toml"), text).unwrap();
        assert!(store.install(source.to_str().unwrap()).is_err());
    }
    fs::remove_file(source.join("Cargo.toml")).unwrap();
    assert!(store.install(source.to_str().unwrap()).is_err());
}

#[test]
fn traversal_and_unconfigured_registry_are_rejected() {
    let temp = TempDir::new().unwrap();
    let store = PluginStore::new(temp.path());
    assert!(store.uninstall("../outside").is_err());
    assert!(store.run("../outside", &[]).is_err());
    assert!(store.install("unknown").is_err());
    assert!(store.registry_add("probe", "file:///untrusted").is_err());
    assert!(store.install("probe").is_err());
}

#[test]
fn sqlite_registry_is_persistent_sorted_and_manageable() {
    let temp = TempDir::new().unwrap();
    let store = PluginStore::new(temp.path());
    store
        .registry_add("zeta", "https://example.invalid/zeta.git")
        .unwrap();
    store
        .registry_add("alpha", "https://example.invalid/alpha.git")
        .unwrap();
    store
        .registry_add("alpha", "https://example.invalid/new-alpha.git")
        .unwrap();
    assert_eq!(
        store.registry_list().unwrap(),
        vec![
            (
                "alpha".into(),
                "https://example.invalid/new-alpha.git".into()
            ),
            ("zeta".into(), "https://example.invalid/zeta.git".into())
        ]
    );
    store.registry_remove("alpha").unwrap();
    assert!(store.registry_remove("alpha").is_err());
    assert_eq!(store.registry_list().unwrap().len(), 1);
}

#[test]
fn plugin_metadata_verification_enablement_and_atomic_update() {
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
    store.set_enabled("probe", false).unwrap();
    assert!(
        store
            .run("probe", &[])
            .unwrap_err()
            .to_string()
            .contains("disabled")
    );
    store.set_enabled("probe", true).unwrap();

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

    fs::write(source.join("src/main.rs"), "not valid Rust").unwrap();
    assert!(store.update("probe").is_err());
    let current = store.info("probe").unwrap();
    assert_eq!(current.manifest.version, "0.2.0");
    assert_eq!(current.checksum, good_checksum);
    store.verify(Some("probe")).unwrap();

    assert_eq!(store.rollback("probe").unwrap().version, "0.1.0");
    assert_eq!(store.info("probe").unwrap().manifest.version, "0.1.0");
    assert!(home.join("backups/probe").is_dir());
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
            format!("#!/bin/sh\nprintf '{marker}\\n' >> \"$DM_HOME/hooks.log\"\n"),
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
    store
        .install_with_consent(source.to_str().unwrap(), None, true)
        .unwrap();
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
fn permission_consent_gates_new_capabilities() {
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

    let error = store
        .install(source.to_str().unwrap())
        .unwrap_err()
        .to_string();
    assert!(error.contains("--accept-permissions"), "{error}");
    assert!(!home.join("plugins/probe").exists());

    store
        .install_with_consent(source.to_str().unwrap(), None, true)
        .unwrap();
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
    assert!(
        store
            .update("probe")
            .unwrap_err()
            .to_string()
            .contains("--accept-permissions")
    );
    store.update_with_consent("probe", true).unwrap();
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
fn json_cli_and_reserved_registry_name() {
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
    assert!(
        !dm(&home)
            .args([
                "registry",
                "add",
                "registry",
                "https://example.invalid/registry.git"
            ])
            .output()
            .unwrap()
            .status
            .success()
    );
}

#[test]
fn newer_sqlite_schema_is_rejected() {
    let temp = TempDir::new().unwrap();
    let database = temp.path().join("store.sqlite3");
    rusqlite::Connection::open(database)
        .unwrap()
        .execute_batch("PRAGMA user_version = 3")
        .unwrap();
    let error = PluginStore::new(temp.path()).list().unwrap_err();
    assert!(error.to_string().contains("schema 3 is newer"));
}

#[test]
fn sqlite_schema_migrates_once_and_stays_current() {
    let temp = TempDir::new().unwrap();
    let database = temp.path().join("store.sqlite3");
    rusqlite::Connection::open(&database)
        .unwrap()
        .execute_batch(
            "CREATE TABLE installed_plugins (
                 name TEXT PRIMARY KEY,
                 manifest TEXT NOT NULL,
                 installed_at INTEGER NOT NULL DEFAULT (unixepoch())
             ) STRICT;
             CREATE TABLE registry (
                 name TEXT PRIMARY KEY,
                 source TEXT NOT NULL,
                 updated_at INTEGER NOT NULL DEFAULT (unixepoch())
             ) STRICT;
             PRAGMA user_version = 1;",
        )
        .unwrap();
    let store = PluginStore::new(temp.path());
    store.list().unwrap();
    store.list().unwrap();
    let version: u32 = rusqlite::Connection::open(database)
        .unwrap()
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, 2);
}

#[test]
fn registry_cli_round_trip() {
    let temp = TempDir::new().unwrap();
    let home = temp.path();
    assert!(
        ok(dm(home)
            .args([
                "registry",
                "add",
                "probe",
                "https://example.invalid/probe.git"
            ])
            .output()
            .unwrap())
        .contains("Registered probe")
    );
    assert_eq!(
        ok(dm(home).args(["registry", "list"]).output().unwrap()),
        "probe\thttps://example.invalid/probe.git\n"
    );
    assert!(
        ok(dm(home)
            .args(["registry", "remove", "probe"])
            .output()
            .unwrap())
        .contains("Removed probe")
    );
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
    assert!(ok(dm(&home).arg("list").output().unwrap()).contains("probe\t0.1.0"));
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
fn registry_resolution_and_git_failure_with_fake_transport() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    let store = PluginStore::new(&home);
    store
        .registry_add("probe", "https://example.invalid/probe.git")
        .unwrap();
    store
        .registry_add("wrong", "https://example.invalid/probe.git")
        .unwrap();
    let tools = temp.path().join("tools");
    fs::create_dir(&tools).unwrap();
    let git = tools.join("git");
    fs::write(&git, "#!/bin/sh\nprintf '%s\\n' \"$@\" >> \"$FAKE_GIT_LOG\"\nfor destination do :; done\ncase \" $* \" in *\" clone \"*) cp -R \"$FAKE_GIT_SOURCE\" \"$destination\" ;; *\" rev-parse \"*) printf '%040d\\n' 1 ;; esac\n").unwrap();
    fs::set_permissions(&git, fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::join_paths(
        std::iter::once(tools).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let log = temp.path().join("git.log");
    let command = || {
        let mut cmd = dm(&home);
        cmd.env("PATH", &path)
            .env("FAKE_GIT_SOURCE", &source)
            .env("FAKE_GIT_LOG", &log);
        cmd
    };

    let verified = command()
        .args([
            "registry",
            "add",
            "--verify",
            "reachable",
            "https://example.invalid/reachable.git",
        ])
        .output()
        .unwrap();
    assert!(
        verified.status.success(),
        "{}",
        String::from_utf8_lossy(&verified.stderr)
    );
    assert!(ok(dm(&home).args(["registry", "list"]).output().unwrap()).contains("reachable"));

    let mismatch = command().args(["install", "wrong"]).output().unwrap();
    assert!(!mismatch.status.success());
    assert!(String::from_utf8_lossy(&mismatch.stderr).contains("does not match"));
    ok(command().args(["install", "probe"]).output().unwrap());
    let args = fs::read_to_string(&log).unwrap();
    assert!(args.contains("protocol.allow=never"));
    assert!(args.contains("--\nhttps://example.invalid/probe.git\n"));
    fs::write(&git, "#!/bin/sh\nexit 1\n").unwrap();
    let failed = command()
        .args(["install", "https://example.invalid/fail.git"])
        .output()
        .unwrap();
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("Git could not fetch"));
    assert_eq!(fs::read_dir(home.join("plugins")).unwrap().count(), 1);

    let bad_verify = command()
        .args([
            "registry",
            "add",
            "--verify",
            "bad",
            "https://example.invalid/bad.git",
        ])
        .output()
        .unwrap();
    assert!(!bad_verify.status.success());
    assert!(!ok(dm(&home).args(["registry", "list"]).output().unwrap()).contains("bad"));
}

#[test]
fn new_command_scaffolds_a_project() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let destination = temp.path().join("created");
    let output = ok(dm(&home)
        .args(["new", "backup", "--directory"])
        .arg(&destination)
        .output()
        .unwrap());
    assert!(output.contains("Created"), "{output}");
    assert!(destination.join("Cargo.toml").is_file());
    assert!(destination.join("dm-plugin.toml").is_file());
    assert!(destination.join("src/main.rs").is_file());
    let cargo = fs::read_to_string(destination.join("Cargo.toml")).unwrap();
    assert!(cargo.contains(r#"name = "dm-plugin-backup""#));
    assert!(cargo.contains(r#"tag = "v0.2.0""#));
    let reserved = temp.path().join("reserved");
    assert!(
        !dm(&home)
            .args(["new", "registry", "--directory"])
            .arg(&reserved)
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(!reserved.exists());

    let offline = temp.path().join("offline");
    let output = dm(&home)
        .args(["new", "backup", "--directory"])
        .arg(&offline)
        .args(["--generate-lockfile"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!offline.exists());
}

#[test]
fn search_command_filters_registry() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    ok(dm(&home)
        .args([
            "registry",
            "add",
            "backup",
            "https://example.invalid/backup.git",
        ])
        .output()
        .unwrap());
    ok(dm(&home)
        .args([
            "registry",
            "add",
            "tools",
            "https://example.invalid/tools.git",
        ])
        .output()
        .unwrap());
    let output = ok(dm(&home).args(["search", "back"]).output().unwrap());
    assert!(output.contains("backup"));
    assert!(output.contains("https://example.invalid/backup.git"));
    assert!(!output.contains("tools"));
    let json = ok(dm(&home).args(["search", "--json"]).output().unwrap());
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.as_array().unwrap().len(), 2);
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
fn remote_registry_index_sync_and_search() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let index = temp.path().join("registry.json");
    fs::write(
        &index,
        r#"[{"name":"backup","source":"https://example.invalid/backup.git"},{"name":"tools","source":"https://example.invalid/tools.git"}]"#,
    )
    .unwrap();
    let tools = temp.path().join("tools");
    fs::create_dir(&tools).unwrap();
    let curl = tools.join("curl");
    fs::write(
        &curl,
        r#"#!/bin/sh
while [ "$#" -gt 0 ]; do
  case "$1" in
    --output) dest="$2"; shift 2;;
    *) shift;;
  esac
done
cp "$FAKE_REGISTRY_INDEX" "$dest"
"#,
    )
    .unwrap();
    fs::set_permissions(&curl, fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::join_paths(
        std::iter::once(tools).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();

    ok(dm(&home)
        .args([
            "registry",
            "add",
            "stale",
            "https://example.invalid/stale.git",
        ])
        .output()
        .unwrap());

    let mut sync = dm(&home);
    sync.env("PATH", &path).env("FAKE_REGISTRY_INDEX", &index);
    let output = ok(sync
        .args([
            "registry",
            "sync",
            "--prune",
            "https://example.invalid/registry.json",
        ])
        .output()
        .unwrap());
    assert!(output.contains("Synced 2"), "{output}");
    assert!(output.contains("Removed 1 stale entries"), "{output}");

    let mut search = dm(&home);
    search.env("PATH", &path).env("FAKE_REGISTRY_INDEX", &index);
    let output = ok(search
        .args([
            "search",
            "--remote",
            "https://example.invalid/registry.json",
            "back",
        ])
        .output()
        .unwrap());
    assert!(output.contains("backup"));
    assert!(!output.contains("tools"));

    let json = ok(dm(&home)
        .args(["registry", "list", "--json"])
        .output()
        .unwrap());
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    let names: Vec<&str> = parsed
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry[0].as_str().unwrap())
        .collect();
    assert_eq!(names, ["backup", "tools"]);
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
