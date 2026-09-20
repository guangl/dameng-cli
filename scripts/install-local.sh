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
mkdir -p "$install_dir"
install -m 755 "${project_dir}/target/release/dm" "${install_dir}/dm"
echo "Installed dm from local source to ${install_dir}/dm"
