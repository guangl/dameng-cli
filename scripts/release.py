"""Version checks and portable release archives; Python 3.11+."""
import hashlib
import os
from pathlib import Path
import sys
import tarfile
import tomllib
import zipfile


def verify():
    host = tomllib.loads(Path("Cargo.toml").read_text())["package"]["version"]
    sdk = tomllib.loads(Path("crates/dm-plugin-sdk/Cargo.toml").read_text())["package"]["version"]
    if os.environ["RELEASE_TAG"] != f"v{host}" or host != sdk:
        raise SystemExit("Tag, host and SDK versions must match")


def package():
    verify()
    target = os.environ["RELEASE_TARGET"]
    tag = os.environ["RELEASE_TAG"]
    windows = "windows" in target
    binary = "dm.exe" if windows else "dm"
    root = f"dm-{tag}-{target}"
    Path("dist").mkdir(exist_ok=True)
    files = [(Path("target") / target / "release" / binary, binary),
             (Path("LICENSE"), "LICENSE"), (Path("README.md"), "README.md")]
    archive = Path("dist") / (root + (".zip" if windows else ".tar.gz"))
    if windows:
        with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as output:
            for source, name in files:
                output.write(source, f"{root}/{name}")
    else:
        with tarfile.open(archive, "w:gz") as output:
            for source, name in files:
                output.add(source, arcname=f"{root}/{name}")
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    archive.with_name(archive.name + ".sha256").write_text(f"{digest}  {archive.name}\n")


if __name__ == "__main__":
    if len(sys.argv) != 2 or sys.argv[1] not in ("verify", "package"):
        raise SystemExit("Usage: release.py verify|package")
    {"verify": verify, "package": package}[sys.argv[1]]()
