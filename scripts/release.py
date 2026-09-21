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
    plugin = tomllib.loads(Path("plugins/ssh/Cargo.toml").read_text())["package"]["version"]
    if os.environ["RELEASE_TAG"] != f"v{host}" or host != sdk:
        raise SystemExit("Tag, host and SDK versions must match")
    if host != plugin:
        raise SystemExit("Tag, host and dm-plugin-ssh versions must match")


def _write_archive(archive: Path, root: str, files, windows: bool):
    if windows:
        with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as output:
            for source, name in files:
                output.write(source, f"{root}/{name}")
    else:
        with tarfile.open(archive, "w:gz") as output:
            for source, name in files:
                output.add(source, arcname=f"{root}/{name}")
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    archive.with_name(archive.name + ".sha256").write_text(f"{digest}  {archive.name}
")


def package():
    verify()
    target = os.environ["RELEASE_TARGET"]
    tag = os.environ["RELEASE_TAG"]
    windows = "windows" in target
    Path("dist").mkdir(exist_ok=True)

    host_binary = "dm.exe" if windows else "dm"
    host_root = f"dm-{tag}-{target}"
    host_files = [
        (Path("target") / target / "release" / host_binary, host_binary),
        (Path("LICENSE"), "LICENSE"),
        (Path("README.md"), "README.md"),
    ]
    _write_archive(
        Path("dist") / (host_root + (".zip" if windows else ".tar.gz")),
        host_root,
        host_files,
        windows,
    )

    plugin_binary = "dm-ssh.exe" if windows else "dm-ssh"
    plugin_root = f"dm-ssh-{tag}-{target}"
    plugin_files = [
        (Path("target") / target / "release" / plugin_binary, plugin_binary),
        (Path("plugins/ssh/dm-plugin.toml"), "dm-plugin.toml"),
        (Path("LICENSE"), "LICENSE"),
        (Path("README.md"), "README.md"),
    ]
    _write_archive(
        Path("dist") / (plugin_root + (".zip" if windows else ".tar.gz")),
        plugin_root,
        plugin_files,
        windows,
    )


if __name__ == "__main__":
    if len(sys.argv) != 2 or sys.argv[1] not in ("verify", "package"):
        raise SystemExit("Usage: release.py verify|package")
    {"verify": verify, "package": package}[sys.argv[1]]()
