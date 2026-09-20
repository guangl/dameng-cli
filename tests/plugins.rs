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
    assert!(!home.exists());
    assert!(
        ok(dm(&home).arg("install").arg(&source).output().unwrap()).contains("Installed probe")
    );
    assert!(ok(dm(&home).arg("list").output().unwrap()).contains("probe\t0.1.0"));
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
    assert!(String::from_utf8_lossy(&output.stderr).contains("plugin stderr"));
    // A broken plugin must remain removable.
    fs::write(home.join("plugins/probe/dm-plugin.toml"), "broken").unwrap();
    ok(dm(&home).args(["uninstall", "probe"]).output().unwrap());
    assert!(ok(dm(&home).arg("list").output().unwrap()).is_empty());
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
    fs::write(
        temp.path().join("registry.toml"),
        "[plugins]\nprobe = \"file:///untrusted\"\n",
    )
    .unwrap();
    assert!(store.install("probe").is_err());
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
    fs::create_dir_all(home.join("plugins")).unwrap();
    let outside = temp.path().join("outside");
    fs::create_dir(&outside).unwrap();
    symlink(&outside, home.join("plugins/probe")).unwrap();
    let store = PluginStore::new(home);
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
    fs::create_dir(&home).unwrap();
    fs::write(home.join("registry.toml"), "[plugins]\nprobe = \"https://example.invalid/probe.git\"\nwrong = \"https://example.invalid/probe.git\"\n").unwrap();
    let tools = temp.path().join("tools");
    fs::create_dir(&tools).unwrap();
    let git = tools.join("git");
    fs::write(&git, "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$FAKE_GIT_LOG\"\nfor destination do :; done\ncp -R \"$FAKE_GIT_SOURCE\" \"$destination\"\n").unwrap();
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
}
