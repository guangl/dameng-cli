#!/bin/sh
set -eu

repository=${DM_INSTALL_REPO:-guangl/dameng-cli}
install_dir=${DM_INSTALL_DIR:-"${HOME}/.local/bin"}
version=${1:-${DM_INSTALL_VERSION:-latest}}

case "$(uname -s):$(uname -m)" in
    Linux:x86_64|Linux:amd64) target=x86_64-unknown-linux-gnu ;;
    Darwin:arm64|Darwin:aarch64) target=aarch64-apple-darwin ;;
    *)
        echo "dm installer: unsupported platform $(uname -s)/$(uname -m)" >&2
        exit 1
        ;;
esac

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
base_url="https://github.com/${repository}/releases/download/${version}"
work_dir=$(mktemp -d "${TMPDIR:-/tmp}/dm-install.XXXXXXXX")
trap 'rm -rf "$work_dir"' EXIT HUP INT TERM

curl -fsSL "${base_url}/${archive}" -o "${work_dir}/${archive}"
curl -fsSL "${base_url}/${archive}.sha256" -o "${work_dir}/${archive}.sha256"

if command -v sha256sum >/dev/null 2>&1; then
    (cd "$work_dir" && sha256sum -c "${archive}.sha256")
elif command -v shasum >/dev/null 2>&1; then
    expected=$(sed 's/[[:space:]].*$//' "${work_dir}/${archive}.sha256")
    actual=$(shasum -a 256 "${work_dir}/${archive}" | sed 's/[[:space:]].*$//')
    [ "$expected" = "$actual" ] || {
        echo "dm installer: checksum verification failed" >&2
        exit 1
    }
else
    echo "dm installer: sha256sum or shasum is required" >&2
    exit 1
fi

tar -xzf "${work_dir}/${archive}" -C "$work_dir"
mkdir -p "$install_dir"
install -m 755 "${work_dir}/dm-${version}-${target}/dm" "${install_dir}/dm"
echo "Installed dm ${version} to ${install_dir}/dm"
