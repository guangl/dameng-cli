"""Reject shipped GNU Linux binaries requiring glibc newer than 2.28."""
import re
import subprocess
import sys
import tomllib
from pathlib import Path

BASELINE = (2, 28)
# Every GNU Linux target the release builds; musl targets are checked by design
# (they carry no glibc symbol requirements) and are refused here.
GNU_TARGETS = {
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "armv7-unknown-linux-gnueabihf",
}


def required_versions(output: str) -> set[tuple[int, ...]]:
    if "GLIBC_PRIVATE" in output:
        raise ValueError("Binary depends on the private glibc ABI")
    versions = {tuple(map(int, value.split(".")))
                for value in re.findall(r"\bGLIBC_(\d+(?:\.\d+)+)\b", output)}
    if not versions:
        raise ValueError("No glibc version requirements found in GNU binary")
    return versions


def check_binary(path: Path) -> None:
    output = subprocess.run(
        ["readelf", "--version-info", str(path)],
        check=True, capture_output=True, text=True,
    ).stdout
    maximum = max(required_versions(output))
    if maximum > BASELINE:
        raise ValueError(f"{path}: requires GLIBC_{'.'.join(map(str, maximum))}, exceeds 2.28")
    print(f"{path.name}: maximum GLIBC_{'.'.join(map(str, maximum))} <= 2.28")


def binaries(target: str) -> list[Path]:
    if target not in GNU_TARGETS:
        raise ValueError(f"Not a supported GNU Linux target: {target}")
    names = ["dm"]
    for manifest in sorted(Path("plugins").glob("*/dm-plugin.toml")):
        with manifest.open("rb") as file:
            names.append("dm-" + tomllib.load(file)["name"])
    return [Path("target") / target / "release" / name for name in names]


if __name__ == "__main__":
    for binary in binaries(sys.argv[1]):
        check_binary(binary)
