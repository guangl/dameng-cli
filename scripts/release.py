"""Version checks and portable release archives; Python 3.11+."""
import hashlib
import os
from pathlib import Path
import sys
import tarfile
import tomllib
import zipfile


def load_toml(path: Path) -> dict:
    return tomllib.loads(path.read_text())


def version_of(path: Path) -> str:
    version = load_toml(path).get("package", {}).get("version")
    if not isinstance(version, str) or not version.strip():
        raise SystemExit(f"{path} does not declare package.version")
    return version


def parse_version(text: str) -> tuple[int, int, int]:
    core = text.split("+")[0].split("-")[0]
    parts = core.split(".")
    if len(parts) != 3 or not all(part.isdigit() for part in parts):
        raise SystemExit(f"Unsupported version {text!r}; expected x.y.z")
    return int(parts[0]), int(parts[1]), int(parts[2])


def bundled_plugins() -> list[tuple[Path, dict]]:
    """Every plugins/<name> shipping both a manifest and a crate, checked."""
    host = version_of(Path("Cargo.toml"))
    plugins = []
    for manifest_path in sorted(Path("plugins").glob("*/dm-plugin.toml")):
        directory = manifest_path.parent
        if not (directory / "Cargo.toml").is_file():
            continue
        manifest = load_toml(manifest_path)
        name = manifest.get("name")
        if name != directory.name:
            raise SystemExit(
                f"{manifest_path} declares name {name!r}, but the directory is "
                f"called {directory.name!r}"
            )
        version = version_of(directory / "Cargo.toml")
        if version != manifest.get("version"):
            raise SystemExit(
                f"{name}: Cargo.toml version {version} must match the "
                f"dm-plugin.toml version {manifest.get('version')}"
            )
        minimum = manifest.get("min_host_version")
        if minimum and parse_version(minimum) > parse_version(host):
            raise SystemExit(
                f"{name} requires host {minimum}, but this release builds host {host}"
            )
        plugins.append((directory, manifest))
    if not plugins:
        raise SystemExit("No bundled plugin found under plugins/")
    return plugins


def verify() -> tuple[str, list[tuple[Path, dict]]]:
    """Check the tag, the SDK and every bundled plugin before packaging."""
    host = version_of(Path("Cargo.toml"))
    sdk = version_of(Path("crates/dm-plugin-sdk/Cargo.toml"))
    tag = os.environ["RELEASE_TAG"]
    if tag != f"v{host}":
        raise SystemExit(f"Release tag {tag} does not match host version {host}")
    if sdk != host:
        raise SystemExit(f"SDK version {sdk} must match host version {host}")
    plugins = bundled_plugins()
    print(
        f"{tag}: host {host}, "
        + ", ".join(f"{manifest['name']} {manifest['version']}" for _, manifest in plugins)
    )
    return tag, plugins


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
    archive.with_name(archive.name + ".sha256").write_text(f"{digest}  {archive.name}\n")


def package():
    tag, plugins = verify()
    target = os.environ["RELEASE_TARGET"]
    windows = "windows" in target
    Path("dist").mkdir(exist_ok=True)

    host_binary = "dm.exe" if windows else "dm"
    host_root = f"dm-{tag}-{target}"
    _write_archive(
        Path("dist") / (host_root + (".zip" if windows else ".tar.gz")),
        host_root,
        [
            (Path("target") / target / "release" / host_binary, host_binary),
            (Path("LICENSE"), "LICENSE"),
            (Path("README.md"), "README.md"),
        ],
        windows,
    )

    for directory, manifest in plugins:
        name = manifest["name"]
        binary = f"dm-{name}" + (".exe" if windows else "")
        plugin_root = f"dm-{name}-{tag}-{target}"
        # A plugin documents itself; fall back to the host files for what is missing.
        contents = {
            binary: Path("target") / target / "release" / binary,
            "dm-plugin.toml": directory / "dm-plugin.toml",
        }
        for extra in ("README.md", "config.example.toml", "LICENSE"):
            if (directory / extra).is_file():
                contents[extra] = directory / extra
        contents.setdefault("README.md", Path("README.md"))
        contents.setdefault("LICENSE", Path("LICENSE"))
        _write_archive(
            Path("dist") / (plugin_root + (".zip" if windows else ".tar.gz")),
            plugin_root,
            [(source, name) for name, source in contents.items()],
            windows,
        )


if __name__ == "__main__":
    if len(sys.argv) != 2 or sys.argv[1] not in ("verify", "package"):
        raise SystemExit("Usage: release.py verify|package")
    {"verify": verify, "package": package}[sys.argv[1]]()
