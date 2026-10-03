#!/usr/bin/env bash
# Build every shipped binary with the same target and GNU glibc baseline.
set -euo pipefail
target=${1:?Usage: build_release.sh TARGET}
builder=(cargo build)
build_target=$target
case "$target" in
  # GNU targets, including the armv7 hard-float triple, keep the 2.28 baseline.
  *-unknown-linux-gnu*)
    builder=(cargo zigbuild)
    build_target="$target.2.28"
    ;;
esac
"${builder[@]}" --release --locked --bin dm --target "$build_target"
for manifest in plugins/*/dm-plugin.toml; do
  directory=$(dirname "$manifest")
  name=$(python3 -c 'import tomllib,sys; print(tomllib.load(open(sys.argv[1], "rb"))["name"])' "$manifest")
  "${builder[@]}" --release --locked --bin "dm-$name" --manifest-path "$directory/Cargo.toml" --target "$build_target"
done
