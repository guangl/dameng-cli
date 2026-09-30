#!/bin/sh
set -eu

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
install_dir=${DM_INSTALL_DIR:-"${HOME}/.local/bin"}

if ! command -v cargo >/dev/null 2>&1; then
    echo "dm local installer: Rust and Cargo are required" >&2
    exit 1
fi

# --quiet keeps Cargo's "Compiling"/"Finished" chatter out of the installer output;
# build warnings and errors still reach stderr, and a failing build stops the script.
echo "Building dm and the ssh/db plugins"
cargo build --quiet --release --locked --bins \
    -p dameng-cli -p dm-plugin-ssh -p dm-plugin-db \
    --manifest-path "${project_dir}/Cargo.toml" --target-dir "${project_dir}/target"
mkdir -p "$install_dir"

# Publish beside the destination with an atomic rename. Parallel installers must
# never expose an absent or partially copied plugin binary in the source package.
staged_binary=''
cleanup() {
    [ -z "$staged_binary" ] || rm -f "$staged_binary"
}
trap cleanup 0
trap 'exit 1' HUP INT TERM
publish_binary() {
    staged_binary=$(mktemp "${2}.XXXXXX")
    install -m 755 "$1" "$staged_binary"
    mv -f "$staged_binary" "$2"
    staged_binary=''
}
publish_binary "${project_dir}/target/release/dm" "${install_dir}/dm"
publish_binary "${project_dir}/target/release/dm-ssh" "${project_dir}/plugins/ssh/dm-ssh"
publish_binary "${project_dir}/target/release/dm-db" "${project_dir}/plugins/db/dm-db"
# --replace keeps the plugin's config/data/cache directories and also works for a
# first installation, so running the installer again upgrades in place. It is also
# the only path that repairs a store written by an older dm whose recorded manifest
# no longer parses (for example one that still carries the removed `permissions`
# field): there "dm info <name>" fails for an installed plugin as well, so an
# info-based check would fall through to a plain install and report the plugin as
# already installed.
"${install_dir}/dm" install "${project_dir}/plugins/ssh" --replace
"${install_dir}/dm" install "${project_dir}/plugins/db" --replace
echo "Installed dm, dm-plugin-ssh and dm-plugin-db from local source to ${install_dir}/dm"
