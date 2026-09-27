#!/bin/sh
# Every Rust file in this repository stays within 200 lines.
#
# The code is organised as small modules so a single file never mixes several
# responsibilities; this check keeps that property from eroding.
set -eu

limit=200
violations=$(git ls-files --cached --others --exclude-standard '*.rs' | while IFS= read -r file; do
    # A file can still be listed while a rename is in flight.
    [ -f "$file" ] || continue
    lines=$(wc -l <"$file" | tr -d ' ')
    if [ "$lines" -gt "$limit" ]; then
        printf '%s: %s lines\n' "$file" "$lines"
    fi
done)

if [ -n "$violations" ]; then
    printf '%s\n' "$violations" >&2
    echo "error: every Rust file must stay within $limit lines; split it into modules." >&2
    exit 1
fi

echo "All Rust files are within $limit lines."
