//! Prebuilt downloads verify the executable against GitHub metadata.

use crate::common::*;
use std::fs;
use tempfile::TempDir;

#[cfg(unix)]
#[test]
fn prebuilt_install_downloads_only_the_binary() {
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
    let metadata = temp.path().join("release.json");
    prebuilt_metadata(&metadata, "probe", b"prebuilt-binary");
    let curl = tools.join("curl");
    fs::write(
        &curl,
        r#"#!/bin/sh
out=""
url=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --output) out="$2"; shift 2;;
    -w) shift 2;;
    *) url="$1"; shift;;
  esac
done
printf '%s\n' "$url" >> "$REQUESTS"
case "$url" in
  */releases/tags/*) cp "$FAKE_RELEASE" "$out";;
  *.sha256) exit 99;;
  *) cp "$FAKE_BIN" "$out";;
esac
printf 200
"#,
    )
    .unwrap();
    fs::set_permissions(&curl, fs::Permissions::from_mode(0o755)).unwrap();

    let path = std::env::join_paths(
        std::iter::once(tools).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    for digest in [
        serde_json::Value::Null,
        serde_json::json!("sha256:bad"),
        serde_json::json!(format!("sha256:{}", "0".repeat(64))),
    ] {
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&metadata).unwrap()).unwrap();
        value["assets"][0]["digest"] = digest;
        fs::write(&metadata, serde_json::to_vec(&value).unwrap()).unwrap();
        let rejected = dm(&home)
            .env("PATH", &path)
            .env("FAKE_GIT_SOURCE", &source)
            .env("FAKE_RELEASE", &metadata)
            .env("FAKE_BIN", &fake_bin)
            .env("REQUESTS", temp.path().join("requests"))
            .args(["install", "https://github.com/example/probe.git"])
            .output()
            .unwrap();
        assert!(!rejected.status.success());
        assert!(!home.join("plugins/probe").exists());
    }
    prebuilt_metadata(&metadata, "probe", b"prebuilt-binary");
    fs::remove_file(temp.path().join("requests")).unwrap();
    let output = dm(&home)
        .env("PATH", &path)
        .env("FAKE_GIT_SOURCE", &source)
        .env("FAKE_RELEASE", &metadata)
        .env("FAKE_BIN", &fake_bin)
        .env("REQUESTS", temp.path().join("requests"))
        .args(["install", "https://github.com/example/probe.git"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let requests = fs::read_to_string(temp.path().join("requests")).unwrap();
    assert_eq!(requests.lines().count(), 2);
    assert!(requests.contains("/dm-probe-"));
    assert!(!requests.contains(".sha256"));
    assert!(requests.contains("api.github.com"));
    let installed = home.join("plugins/probe/dm-probe");
    assert_eq!(fs::read(installed).unwrap(), b"prebuilt-binary");
    let info = ok(dm(&home).args(["info", "probe"]).output().unwrap());
    assert!(!info.is_empty());
}
