"""Version checks and portable release archives; Python 3.11+."""
import hashlib
import os
from pathlib import Path
import sys
import tarfile
import tomllib
import zipfile

# Plugin protocol versions this host understands; keep in sync with the SDK.
SUPPORTED_API_VERSIONS = {1}
# Name of the asset listing the bundled plugins; read by scripts/install.sh.
PLUGIN_LIST = "dm-plugins-{tag}-{target}.txt"


def load_toml(path: Path) -> dict:
    return tomllib.loads(path.read_text())


def version_of(path: Path) -> str:
    version = load_toml(path).get("package", {}).get("version")
    if not isinstance(version, str) or not version.strip():
        raise SystemExit(f"{path} does not declare package.version")
    return version


def parse_version(text: str) -> tuple[int, int, int, tuple[str, ...]]:
    """Parse SemVer into (major, minor, patch, prerelease identifiers).

    Build metadata is ignored; an empty prerelease tuple outranks every
    prerelease, which is the SemVer rule for 1.0.0 against 1.0.0-rc.1.
    """
    core, separator, prerelease = text.partition("+")[0].partition("-")
    if separator and not prerelease:
        raise SystemExit(f"Unsupported version {text!r}; empty prerelease")
    parts = core.split(".")
    if len(parts) != 3 or not all(part.isdigit() for part in parts):
        raise SystemExit(f"Unsupported version {text!r}; expected SemVer like 1.2.3")
    identifiers: tuple[str, ...] = ()
    if prerelease:
        identifiers = tuple(prerelease.split("."))
        if not all(
            identifier
            and all(character.isalnum() or character == "-" for character in identifier)
            for identifier in identifiers
        ):
            raise SystemExit(f"Unsupported prerelease in version {text!r}")
    return int(parts[0]), int(parts[1]), int(parts[2]), identifiers


def compare_versions(left: str, right: str) -> int:
    """Compare two SemVer strings, returning -1, 0 or 1 like a comparator."""
    left_major, left_minor, left_patch, left_pre = parse_version(left)
    right_major, right_minor, right_patch, right_pre = parse_version(right)
    left_core = (left_major, left_minor, left_patch)
    right_core = (right_major, right_minor, right_patch)
    if left_core != right_core:
        return -1 if left_core < right_core else 1
    if left_pre == right_pre:
        return 0
    # A version without prerelease outranks the same version with one.
    if not left_pre:
        return 1
    if not right_pre:
        return -1
    for left_id, right_id in zip(left_pre, right_pre):
        if left_id == right_id:
            continue
        left_numeric = left_id.isdigit()
        right_numeric = right_id.isdigit()
        if left_numeric and right_numeric:
            return -1 if int(left_id) < int(right_id) else 1
        if left_numeric != right_numeric:
            # Numeric identifiers sort below alphanumeric ones.
            return -1 if left_numeric else 1
        return -1 if left_id < right_id else 1
    return -1 if len(left_pre) < len(right_pre) else 1


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
        api_version = manifest.get("api_version")
        if api_version not in SUPPORTED_API_VERSIONS:
            raise SystemExit(
                f"{name}: api_version {api_version!r} is not supported by this host; "
                f"expected one of {sorted(SUPPORTED_API_VERSIONS)}"
            )
        minimum = manifest.get("min_host_version")
        if minimum and compare_versions(minimum, host) > 0:
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


def plugin_contents(
    directory: Path, manifest: dict, target: str, windows: bool
) -> list[tuple[Path, str]]:
    """Files of one plugin archive as (source, name inside the archive)."""
    name = manifest["name"]
    binary = f"dm-{name}" + (".exe" if windows else "")
    contents = {
        binary: Path("target") / target / "release" / binary,
        "dm-plugin.toml": directory / "dm-plugin.toml",
    }
    for extra in ("README.md", "config.example.toml", "LICENSE"):
        if (directory / extra).is_file():
            contents[extra] = directory / extra
    contents.setdefault("README.md", Path("README.md"))
    contents.setdefault("LICENSE", Path("LICENSE"))
    # The host runs pre_install from the extracted archive and copies the other
    # declared hooks, so every declared hook has to travel inside the archive.
    for hook in (manifest.get("hooks") or {}).values():
        source = directory / hook
        if not source.is_file():
            raise SystemExit(f"{name}: declared hook {hook} does not exist")
        contents[str(hook)] = source
    return [(source, entry) for entry, source in contents.items()]


def _write_checksum(archive: Path):
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    archive.with_name(archive.name + ".sha256").write_text(f"{digest}  {archive.name}\n")


def _write_archive(archive: Path, root: str, files, windows: bool):
    if windows:
        with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as output:
            for source, name in files:
                output.write(source, f"{root}/{name}")
    else:
        with tarfile.open(archive, "w:gz") as output:
            for source, name in files:
                output.add(source, arcname=f"{root}/{name}")
    _write_checksum(archive)


def package():
    tag, plugins = verify()
    target = os.environ["RELEASE_TARGET"]
    windows = "windows" in target
    dist = Path("dist")
    dist.mkdir(exist_ok=True)

    host_binary = "dm.exe" if windows else "dm"
    host_root = f"dm-{tag}-{target}"
    _write_archive(
        dist / (host_root + (".zip" if windows else ".tar.gz")),
        host_root,
        [
            (Path("target") / target / "release" / host_binary, host_binary),
            (Path("LICENSE"), "LICENSE"),
            (Path("README.md"), "README.md"),
        ],
        windows,
    )

    for directory, manifest in plugins:
        plugin_root = f"dm-{manifest['name']}-{tag}-{target}"
        _write_archive(
            dist / (plugin_root + (".zip" if windows else ".tar.gz")),
            plugin_root,
            plugin_contents(directory, manifest, target, windows),
            windows,
        )

    # scripts/install.sh reads this list, so published and installed plugins
    # cannot drift apart.
    listing = dist / PLUGIN_LIST.format(tag=tag, target=target)
    listing.write_text("".join(f"{manifest['name']}\n" for _, manifest in plugins))
    _write_checksum(listing)


if __name__ == "__main__":
    if len(sys.argv) != 2 or sys.argv[1] not in ("verify", "package"):
        raise SystemExit("Usage: release.py verify|package")
    {"verify": verify, "package": package}[sys.argv[1]]()
