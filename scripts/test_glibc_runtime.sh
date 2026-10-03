#!/usr/bin/env bash
# Execute all shipped GNU binaries against Debian 10's glibc 2.28.
set -euo pipefail
target=${1:?Usage: test_glibc_runtime.sh TARGET}
case "$target" in
  x86_64-unknown-linux-gnu) platform=linux/amd64 ;;
  aarch64-unknown-linux-gnu) platform=linux/arm64 ;;
  armv7-unknown-linux-gnueabihf) platform=linux/arm/v7 ;;
  *) echo "Unsupported GNU target: $target" >&2; exit 1 ;;
esac
# CI uses a native runner where one exists and QEMU for ARMv7, so the platform
# is always stated explicitly.
docker run --rm --platform "$platform" --network none --tmpfs /tmp:rw,exec,nosuid,size=256m \
  -e DM_PLUGIN_HOME=/tmp/dm-home \
  -v "$PWD/target/$target/release:/artifacts:ro" \
  -v "$PWD/plugins:/manifests:ro" \
  debian:buster-slim sh -ec '
    ldd --version | head -n 1 | grep -F "2.28"
    /artifacts/dm --version
    for binary in /artifacts/dm-*; do
      [ -f "$binary" ] && [ -x "$binary" ] || continue
      name=${binary##*/dm-}
      mkdir -p "/tmp/package-$name"
      cp "$binary" "/tmp/package-$name/"
      cp "/manifests/$name/dm-plugin.toml" "/tmp/package-$name/"
      /artifacts/dm install "/tmp/package-$name"
      /artifacts/dm "$name" --help
      /artifacts/dm "$name" list
    done
  '
