#![cfg(unix)]
use crate::common::*;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

pub(super) fn source(root: &Path) -> PathBuf {
    let source = root.join("source with spaces");
    fs::create_dir_all(source.join("src")).unwrap();
    fs::write(source.join("dm-plugin.toml"), manifest("probe")).unwrap();
    fs::write(source.join("Cargo.toml"), "[package]\nname='probe'\nversion='0.1.0'\nedition='2024'\nrust-version='1.99.0'\n[[bin]]\nname='dm-probe'\npath='src/main.rs'\n").unwrap();
    fs::write(
        source.join("Cargo.lock"),
        "version=4\n[[package]]\nname='probe'\nversion='0.1.0'\n",
    )
    .unwrap();
    fs::write(
        source.join("src/main.rs"),
        "fn main() { println!(\"compiled plugin\"); }\n",
    )
    .unwrap();
    source
}

pub(super) fn tools(root: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let tools = root.join("tools");
    fs::create_dir_all(&tools).unwrap();
    let rustup = tools.join("rustup");
    fs::write(
        &rustup,
        r#"#!/bin/sh
printf '%s\n' "$*" >> "$BUILD_CALLS"
case "$1" in
 which) [ "$MISSING_TOOLCHAIN" != 1 ]; exit $?;;
 toolchain) exit "${INSTALL_EXIT:-0}";;
 run)
  [ "$BUILD_EXIT" != 1 ] || exit 1
  if [ "$REAL_BUILD" = 1 ]; then shift 2; exec "$REAL_RUSTUP" run stable "$@"; fi
  [ "$NO_BINARY" != 1 ] || exit 0
  shift 2
  while [ "$#" -gt 0 ]; do
   case "$1" in
    --target-dir) target_dir="$2"; shift 2;;
    --target) target="$2"; shift 2;;
    *) shift;;
   esac
  done
  mkdir -p "$target_dir/$target/release"
  printf '#!/bin/sh\nprintf "compiled plugin\\n"\n' > "$target_dir/$target/release/dm-probe"
  exit 0;;
esac
exit 2
"#,
    )
    .unwrap();
    fs::set_permissions(rustup, fs::Permissions::from_mode(0o755)).unwrap();
    tools
}

pub(super) fn command(home: &Path, tools: &Path, root: &Path) -> Command {
    let mut command = dm(home);
    command
        .env("PATH", prepend_tools_to_path(tools))
        .env("BUILD_CALLS", root.join("calls"))
        .env_remove("DM_BUILD_TOOLCHAIN")
        .env_remove("BUILD_EXIT")
        .env_remove("MISSING_TOOLCHAIN")
        .env_remove("INSTALL_EXIT")
        .env_remove("NO_BINARY")
        .env_remove("REAL_BUILD");
    command
}

pub(super) fn calls(root: &Path) -> String {
    fs::read_to_string(root.join("calls")).unwrap_or_default()
}
