#!/bin/sh
set -eu

repository=${DM_INSTALL_REPO:-guangl/dameng-cli}
install_dir=${DM_INSTALL_DIR:-"${HOME}/.local/bin"}
version=${1:-${DM_INSTALL_VERSION:-latest}}

if [ -n "${DM_INSTALL_TARGET:-}" ]; then
    target=${DM_INSTALL_TARGET}
else
    case "$(uname -s):$(uname -m)" in
        Linux:x86_64|Linux:amd64) target=x86_64-unknown-linux-gnu ;;
        Linux:aarch64|Linux:arm64) target=aarch64-unknown-linux-gnu ;;
        Darwin:arm64|Darwin:aarch64) target=aarch64-apple-darwin ;;
        *)
            echo "dm installer: unsupported platform $(uname -s)/$(uname -m)" >&2
            exit 1
            ;;
    esac
fi

if ! command -v curl >/dev/null 2>&1; then
    echo "dm installer: curl is required" >&2
    exit 1
fi

if [ "$version" = latest ]; then
    release_url=$(curl -fsSIL -o /dev/null -w '%{url_effective}' \
        "https://github.com/${repository}/releases/latest")
    version=${release_url##*/}
fi

case "$version" in
    v[0-9]* ) ;;
    *)
        echo "dm installer: version must look like v1.2.3" >&2
        exit 1
        ;;
esac
case "$version" in
    *[!A-Za-z0-9._-]*)
        echo "dm installer: version contains unsupported characters" >&2
        exit 1
        ;;
esac

archive="dm-${version}-${target}.tar.gz"
plugin_list="dm-plugins-${version}-${target}.txt"
base_url="https://github.com/${repository}/releases/download/${version}"
work_dir=$(mktemp -d "${TMPDIR:-/tmp}/dm-install.XXXXXXXX")
trap 'rm -rf "$work_dir"' EXIT HUP INT TERM

# Verify one downloaded file against the SHA-256 sidecar published with it.
verify_sha256() {
    checked=$1
    if command -v sha256sum >/dev/null 2>&1; then
        (cd "$work_dir" && sha256sum -c "${checked}.sha256")
    elif command -v shasum >/dev/null 2>&1; then
        expected=$(sed 's/[[:space:]].*$//' "$work_dir/${checked}.sha256")
        actual=$(shasum -a 256 "$work_dir/${checked}" | sed 's/[[:space:]].*$//')
        [ "$expected" = "$actual" ] || {
            echo "dm installer: checksum verification failed for ${checked}" >&2
            exit 1
        }
    else
        echo "dm installer: sha256sum or shasum is required" >&2
        exit 1
    fi
}

# Download one release asset into the work directory and verify its checksum.
# Returns 1 when the release does not publish the asset at all, and stops the
# installation for every other transport, HTTP or checksum error.
download_asset() {
    asset=$1
    status=$(curl -sSL -o "$work_dir/${asset}" -w '%{http_code}' "${base_url}/${asset}") || {
        echo "dm installer: cannot download ${asset}" >&2
        exit 1
    }
    case "$status" in
        200) ;;
        404)
            rm -f "$work_dir/${asset}"
            return 1
            ;;
        *)
            echo "dm installer: unexpected HTTP ${status} for ${asset}" >&2
            exit 1
            ;;
    esac
    status=$(curl -sSL -o "$work_dir/${asset}.sha256" -w '%{http_code}' "${base_url}/${asset}.sha256") || {
        echo "dm installer: cannot download ${asset}.sha256" >&2
        exit 1
    }
    [ "$status" = 200 ] || {
        echo "dm installer: ${asset}.sha256 is missing (HTTP ${status})" >&2
        exit 1
    }
    verify_sha256 "$asset"
    return 0
}

if ! download_asset "$archive"; then
    echo "dm installer: ${archive} is not published for ${version}" >&2
    exit 1
fi
tar -xzf "$work_dir/${archive}" -C "$work_dir"
mkdir -p "$install_dir"
install -m 755 "$work_dir/dm-${version}-${target}/dm" "$install_dir/dm"

# Bundled plugins. The release publishes ${plugin_list}, so packaged and
# installed plugins cannot drift apart; tags cut before that list exist fall back
# to the names below, and a plugin a tag does not publish is skipped with a
# notice instead of failing the whole installation.
plugins="ssh db"
if download_asset "$plugin_list"; then
    plugins=$(sed -e 's/[[:space:]]*$//' -e '/^$/d' "$work_dir/${plugin_list}")
fi

# Word splitting is intended: the list holds one plugin name per line.
for plugin in $plugins; do
    plugin_archive="dm-${plugin}-${version}-${target}.tar.gz"
    if ! download_asset "$plugin_archive"; then
        echo "dm installer: dm-${plugin} is not published for ${version}; skipping" >&2
        continue
    fi
    tar -xzf "$work_dir/${plugin_archive}" -C "$work_dir"
    plugin_dir="$work_dir/dm-${plugin}-${version}-${target}"
    # --replace keeps the plugin's config/data/cache directories and also works
    # for a first installation, so running the installer again upgrades in place.
    "$install_dir/dm" install "$plugin_dir" --replace
done
echo "Installed dm ${version} and its bundled plugins to $install_dir/dm"
