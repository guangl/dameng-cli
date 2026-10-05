//! Shared fixtures and helpers for the integration target.
//!
//! Modules import them with `use crate::common::*;`; the config modules alias
//! the isolated `dm_isolated` variant as `dm`.

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::{Command, Output};

pub(crate) fn dm(home: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_dm"));
    command
        .env("DM_PLUGIN_HOME", home)
        .env("CARGO_NET_OFFLINE", "true");
    command
}
pub(crate) fn dm_isolated(home: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_dm"));
    command
        .env("DM_PLUGIN_HOME", home)
        .env_remove("DM_LOG")
        .env_remove("RUST_LOG")
        .env_remove("DM_UPDATE_REPOSITORY");
    command
}
pub(crate) fn ok(output: Output) -> String {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}
pub(crate) fn manifest(name: &str) -> String {
    format!("name = {name:?}\nversion = \"0.1.0\"\ndescription = \"test\"\napi_version = 1\n")
}
pub(crate) fn fixture(root: &Path) -> PathBuf {
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
pub(crate) fn write_config(home: &Path, text: &str) {
    fs::write(home.join("config.toml"), text).unwrap();
}
pub(crate) fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}
pub(crate) fn write_fake_curl(tools: &std::path::Path) {
    let curl = tools.join("curl");
    fs::write(
        &curl,
        r#"#!/bin/sh
dest=""
url=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --output) dest="$2"; shift 2;;
    *) url="$1"; shift;;
  esac
done
case "$url" in
  */releases/tags/*) printf '{"assets":[{"name":"dm-%s-%s.tar.gz","digest":"sha256:%s"}]}' "$FAKE_TAG" "$FAKE_TARGET" "$FAKE_DIGEST" > "$dest"; printf 200;;
  *latest) printf '{"tag_name":"v9.9.9"}' > "$dest";;
  *) cp "$FAKE_ARCHIVE" "$dest";;
esac
"#,
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&curl, fs::Permissions::from_mode(0o755)).unwrap();
    }
}
pub(crate) fn write_fake_tar(tools: &std::path::Path) {
    let tar = tools.join("tar");
    fs::write(
        &tar,
        r#"#!/bin/sh
root=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    -C) root="$2"; shift 2;;
    *) shift;;
  esac
done
mkdir -p "$root/dm-$FAKE_TAG-$FAKE_TARGET"
printf 'new-dm-binary' > "$root/dm-$FAKE_TAG-$FAKE_TARGET/dm"
"#,
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tar, fs::Permissions::from_mode(0o755)).unwrap();
    }
}
pub(crate) fn prepend_tools_to_path(tools: &std::path::Path) -> std::ffi::OsString {
    std::env::join_paths(
        std::iter::once(tools.to_path_buf())
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap()
}

pub(crate) fn prebuilt_metadata(path: &Path, name: &str, binary: &[u8]) {
    use sha2::{Digest, Sha256};
    let target = dameng_cli::prebuilt_target_label_for(env!("DM_HOST_TARGET")).unwrap();
    let metadata = serde_json::json!({"assets":[{"name":format!("dm-{name}-{target}{}", std::env::consts::EXE_SUFFIX), "digest":format!("sha256:{}", dameng_cli::support::codec::hex(&Sha256::digest(binary)))}]});
    fs::write(path, serde_json::to_vec(&metadata).unwrap()).unwrap();
}
