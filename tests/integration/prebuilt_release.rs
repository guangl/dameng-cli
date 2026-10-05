//! Prebuilt releases take precedence over building from source.

use crate::common::*;
use std::fs;
use tempfile::TempDir;

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
    let metadata = temp.path().join("release.json");
    prebuilt_metadata(&metadata, "probe", b"prebuilt-binary");
    let curl = tools.join("curl");
    fs::write(
        &curl,
        "#!/bin/sh\nout=\nurl=\nwhile [ \"$#\" -gt 0 ]; do\n  case \"$1\" in\n    --output) out=\"$2\"; shift 2;;\n    -w) shift 2;;\n    *) url=\"$1\"; shift;;\n  esac\ndone\ncase \"$url\" in\n  */releases/tags/*) cp \"$FAKE_RELEASE\" \"$out\"; printf 200;;\n  *) cp \"$FAKE_BIN\" \"$out\"; printf 200;;\nesac\n",
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
        .env("FAKE_RELEASE", &metadata)
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
    assert!(!stderr.contains("SHA-256 sidecar"), "{stderr}");
    assert!(
        !stderr.contains("Building probe 0.1.0 from source"),
        "{stderr}"
    );
    let installed = home
        .join("plugins/probe")
        .join(format!("dm-probe{}", std::env::consts::EXE_SUFFIX));
    assert_eq!(fs::read(installed).unwrap(), b"prebuilt-binary");
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
        String::from_utf8_lossy(&output.stderr).contains("metadata download failed"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
