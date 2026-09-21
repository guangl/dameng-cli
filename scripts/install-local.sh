#!/bin/sh
set -eu

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
install_dir=${DM_INSTALL_DIR:-"${HOME}/.local/bin"}

if ! command -v cargo >/dev/null 2>&1; then
    echo "dm local installer: Rust and Cargo are required" >&2
    exit 1
fi

cargo build --release --locked --bin dm --manifest-path "${project_dir}/Cargo.toml" \
    --target-dir "${project_dir}/target"
cargo build --release --locked --bin dm-ssh --manifest-path "${project_dir}/plugins/ssh/Cargo.toml" \
    --target-dir "${project_dir}/target"
mkdir -p "$install_dir"
install -m 755 "${project_dir}/target/release/dm" "${install_dir}/dm"
install -m 755 "${project_dir}/target/release/dm-ssh" "${project_dir}/plugins/ssh/dm-ssh"
if "${install_dir}/dm" info ssh >/dev/null 2>&1; then
    "${install_dir}/dm" update ssh
else
    "${install_dir}/dm" install "${project_dir}/plugins/ssh"
fi
echo "Installed dm and dm-plugin-ssh from local source to ${install_dir}/dm"
