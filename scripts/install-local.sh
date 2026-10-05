#!/bin/sh
set -eu

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
install_dir=${DM_INSTALL_DIR:-"${HOME}/.local/bin"}

set -f
plugins=$(printf '%s' "${DM_INSTALL_PLUGINS-ssh db}" | tr ',' ' ')
for plugin in $plugins; do
    case "$plugin" in
        ssh|db|sqllog2db) ;;
        *) echo "dm local installer: unknown plugin: $plugin (available: ssh db sqllog2db)" >&2; exit 1 ;;
    esac
done
plugins=$(printf '%s\n' $plugins | awk 'NF && !seen[$0]++')

if ! command -v cargo >/dev/null 2>&1; then
    echo "dm local installer: Rust and Cargo are required" >&2
    exit 1
fi

manifests="crates/dm-plugin-sdk/Cargo.toml"
packages=""
for plugin in $plugins; do
    [ "$plugin" != sqllog2db ] || continue
    manifests="$manifests plugins/$plugin/Cargo.toml"
    packages="$packages -p dm-plugin-$plugin"
done
for manifest in $manifests; do
    [ -f "$project_dir/$manifest" ] || {
        echo "dm local installer: initialize components with git submodule update --init --recursive" >&2
        exit 1
    }
done

# --quiet keeps Cargo's "Compiling"/"Finished" chatter out of the installer output;
# build warnings and errors still reach stderr, and a failing build stops the script.
echo "Building dm; selected plugins: ${plugins:-none}"
cargo build --quiet --release --locked --bins \
    -p dameng-cli $packages \
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
for plugin in $plugins; do
    if [ "$plugin" = sqllog2db ]; then
        "${install_dir}/dm" install https://github.com/guangl/dm-database-sqllog2db.git --rev v3.0.2 --replace
        continue
    fi
    publish_binary "${project_dir}/target/release/dm-$plugin" "${project_dir}/plugins/$plugin/dm-$plugin"
    "${install_dir}/dm" install "${project_dir}/plugins/$plugin" --replace
done
for shell in bash zsh; do
    "${install_dir}/dm" completions "${shell}" --install ||
        echo "dm local installer: ${shell} completion was not installed" >&2
done
echo "Installed dm and selected plugins from local source to ${install_dir}/dm"
