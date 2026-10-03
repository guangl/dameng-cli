"""Tests for scripts/release.py; run with python3 scripts/test_release.py."""
import contextlib
import io
import os
import sys
import tarfile
import tempfile
import unittest
from unittest import mock
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import release  # noqa: E402  (imported after the module path is set up)


def write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)


def run_quietly(function, *args):
    """Run a release.py entry point without its informational output."""
    with contextlib.redirect_stdout(io.StringIO()):
        return function(*args)


def manifest_text(
    name: str,
    version: str = "0.1.0",
    api_version: int = 1,
    min_host_version: str | None = None,
    hooks: dict | None = None,
) -> str:
    lines = [f'name = "{name}"', f'version = "{version}"', f"api_version = {api_version}"]
    if min_host_version is not None:
        lines.append(f'min_host_version = "{min_host_version}"')
    if hooks:
        lines.append("[hooks]")
        lines.extend(f'{key} = "{path}"' for key, path in hooks.items())
    return "\n".join(lines) + "\n"


class EncodingTests(unittest.TestCase):
    def test_toml_is_utf8_even_when_default_text_reads_use_windows_encoding(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "plugin.toml"
            path.write_bytes('name = "db"\ndescription = "插件配置"\n'.encode("utf-8"))
            original = Path.read_text
            def windows_read(path, *args, **kwargs):
                kwargs.setdefault("encoding", "cp1252")
                return original(path, *args, **kwargs)
            with mock.patch.object(Path, "read_text", windows_read):
                self.assertEqual(release.load_toml(path)["description"], "插件配置")


class Repo:
    """A throwaway repository tree that release.py can run inside."""

    def __init__(self, root: Path, host: str = "0.2.0", plugins=("alpha", "beta")):
        self.root = root
        write(root / "Cargo.toml", f'[package]\nname = "host"\nversion = "{host}"\n')
        write(
            root / "crates/dm-plugin-sdk/Cargo.toml",
            f'[package]\nname = "sdk"\nversion = "{host}"\n',
        )
        write(root / "LICENSE", "MIT\n")
        write(root / "README.md", "# host\n")
        for name in plugins:
            write(
                root / f"plugins/{name}/Cargo.toml",
                f'[package]\nname = "dm-plugin-{name}"\nversion = "0.1.0"\nrepository = "https://github.com/example/dm-plugin-{name}"\n',
            )
            write(root / f"plugins/{name}/dm-plugin.toml", manifest_text(name))
            write(root / f"plugins/{name}/README.md", f"# {name}\n")

    def binaries(self, target: str, names=("dm", "dm-alpha", "dm-beta")):
        for name in names:
            write(self.root / f"target/{target}/release/{name}", "binary\n")


class TempRepoTest(unittest.TestCase):
    host = "0.2.0"

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.previous = Path.cwd()
        os.chdir(self.temp.name)
        self.repo = Repo(Path(self.temp.name), host=self.host)
        self.tag = f"v{self.host}"
        os.environ["RELEASE_TAG"] = self.tag
        os.environ["RELEASE_TARGET"] = "aarch64-apple-darwin"

    def tearDown(self):
        os.chdir(self.previous)
        self.temp.cleanup()


class VersionTests(unittest.TestCase):
    def test_parse_version_keeps_prerelease_identifiers(self):
        self.assertEqual(release.parse_version("1.2.3"), (1, 2, 3, ()))
        self.assertEqual(release.parse_version("1.2.3-rc.1"), (1, 2, 3, ("rc", "1")))
        # Build metadata does not take part in precedence.
        self.assertEqual(release.parse_version("1.2.3-rc.1+build.5"), (1, 2, 3, ("rc", "1")))

    def test_parse_version_rejects_unsupported_text(self):
        for text in ("1.2", "1.2.3.4", "v1.2.3", "1.2.x", "1.2.3-", "1.2.3-rc..1"):
            with self.subTest(text=text), self.assertRaises(SystemExit):
                release.parse_version(text)

    def test_compare_versions_follows_semver_precedence(self):
        for left, right, expected in [
            ("1.0.0", "1.0.0", 0),
            ("1.0.0", "2.0.0", -1),
            ("1.1.0", "1.0.9", 1),
            # A prerelease sorts below its release, the case that made a
            # prerelease host pass a min_host_version check it must fail.
            ("0.2.0-rc.1", "0.2.0", -1),
            ("0.2.0", "0.2.0-rc.1", 1),
            ("1.0.0-alpha", "1.0.0-alpha.1", -1),
            ("1.0.0-alpha.1", "1.0.0-alpha.beta", -1),
            ("1.0.0-alpha.beta", "1.0.0-beta", -1),
            ("1.0.0-beta.2", "1.0.0-beta.11", -1),
            ("1.0.0-1", "1.0.0-alpha", -1),
            ("1.0.0+build", "1.0.0", 0),
        ]:
            with self.subTest(left=left, right=right):
                self.assertEqual(release.compare_versions(left, right), expected)
                self.assertEqual(release.compare_versions(right, left), -expected)


class PluginGateTests(TempRepoTest):
    def manifest(self, name: str) -> Path:
        return Path("plugins") / name / "dm-plugin.toml"

    def test_consistent_tree_is_accepted(self):
        tag, plugins = run_quietly(release.verify)
        self.assertEqual(tag, self.tag)
        self.assertEqual([manifest["name"] for _, manifest in plugins], ["alpha", "beta"])

    def test_unsupported_api_version_is_rejected(self):
        self.manifest("alpha").write_text(manifest_text("alpha", api_version=2))
        with self.assertRaises(SystemExit) as raised:
            release.verify()
        self.assertIn("api_version 2", str(raised.exception))

    def test_plugin_crate_version_must_match_its_manifest(self):
        write(Path("plugins/alpha/Cargo.toml"), '[package]\nname = "dm-plugin-alpha"\nversion = "0.9.9"\n')
        with self.assertRaises(SystemExit) as raised:
            release.verify()
        self.assertIn("0.9.9", str(raised.exception))

    def test_directory_name_must_match_the_manifest_name(self):
        self.manifest("alpha").write_text(manifest_text("gamma"))
        with self.assertRaises(SystemExit) as raised:
            release.verify()
        self.assertIn("gamma", str(raised.exception))

    def test_missing_plugin_is_rejected(self):
        for directory in sorted(Path("plugins").iterdir()):
            for file in directory.iterdir():
                file.unlink()
            directory.rmdir()
        with self.assertRaises(SystemExit) as raised:
            release.verify()
        self.assertIn("No bundled plugin", str(raised.exception))

    def test_release_tag_must_match_the_host_version(self):
        os.environ["RELEASE_TAG"] = "v9.9.9"
        with self.assertRaises(SystemExit) as raised:
            release.verify()
        self.assertIn("v9.9.9", str(raised.exception))

    def test_sdk_version_is_independent_of_the_host_version(self):
        write(Path("crates/dm-plugin-sdk/Cargo.toml"), '[package]\nname = "sdk"\nversion = "0.1.0"\n')
        tag, _ = run_quietly(release.verify)
        self.assertEqual(tag, self.tag)


class PrereleaseHostTests(TempRepoTest):
    host = "0.2.0-rc.1"

    def test_min_host_version_above_a_prerelease_host_is_rejected(self):
        manifest = Path("plugins/alpha/dm-plugin.toml")
        manifest.write_text(manifest_text("alpha", min_host_version="0.2.0"))
        with self.assertRaises(SystemExit) as raised:
            release.verify()
        self.assertIn("requires host 0.2.0", str(raised.exception))

    def test_min_host_version_below_a_prerelease_host_is_accepted(self):
        manifest = Path("plugins/alpha/dm-plugin.toml")
        manifest.write_text(manifest_text("alpha", min_host_version="0.2.0-rc.0"))
        tag, _ = run_quietly(release.verify)
        self.assertEqual(tag, "v0.2.0-rc.1")


class PackagingTests(TempRepoTest):
    target = "aarch64-apple-darwin"

    def test_package_writes_host_plugin_and_list_archives(self):
        self.repo.binaries(self.target)
        run_quietly(release.package)
        dist = Path("dist")
        for name in (f"dm-v0.2.0-{self.target}", f"dm-alpha-v0.2.0-{self.target}"):
            archive = dist / f"{name}.tar.gz"
            self.assertTrue(archive.is_file(), name)
            self.assertTrue(archive.with_name(archive.name + ".sha256").is_file(), name)
        listing = dist / f"dm-plugins-v0.2.0-{self.target}.txt"
        self.assertEqual(listing.read_text(), "alpha\nbeta\n")
        self.assertTrue(listing.with_name(listing.name + ".sha256").is_file())
        sources = dist / f"dm-plugin-sources-v0.2.0-{self.target}.txt"
        self.assertEqual(sources.read_text(),
                         "alpha example/dm-plugin-alpha v0.1.0\nbeta example/dm-plugin-beta v0.1.0\n")
        self.assertTrue(sources.with_name(sources.name + ".sha256").is_file())

        with tarfile.open(dist / f"dm-alpha-v0.2.0-{self.target}.tar.gz") as archive:
            names = archive.getnames()
        self.assertIn(f"dm-alpha-v0.2.0-{self.target}/dm-alpha", names)
        self.assertIn(f"dm-alpha-v0.2.0-{self.target}/dm-plugin.toml", names)
        # The plugin documents itself; the host files fill the gaps.
        self.assertIn(f"dm-alpha-v0.2.0-{self.target}/README.md", names)
        self.assertIn(f"dm-alpha-v0.2.0-{self.target}/LICENSE", names)

    def test_plugin_source_requires_a_github_repository(self):
        for repository in ("https://example.com/owner/repo", "https://github.com/owner/repo/extra"):
            with self.subTest(repository=repository):
                write(Path("plugins/alpha/Cargo.toml"),
                      f'[package]\nversion = "0.1.0"\nrepository = "{repository}"\n')
                self.repo.binaries(self.target)
                with self.assertRaises(SystemExit):
                    run_quietly(release.package)

    def test_declared_hooks_travel_inside_the_archive(self):
        manifest = Path("plugins/alpha/dm-plugin.toml")
        manifest.write_text(manifest_text("alpha", hooks={"pre_install": "hooks/pre-install.sh"}))
        write(Path("plugins/alpha/hooks/pre-install.sh"), "#!/bin/sh\nexit 0\n")
        self.repo.binaries(self.target)
        run_quietly(release.package)

        with tarfile.open(Path("dist") / f"dm-alpha-v0.2.0-{self.target}.tar.gz") as archive:
            names = archive.getnames()
        self.assertIn(f"dm-alpha-v0.2.0-{self.target}/hooks/pre-install.sh", names)

    def test_missing_declared_hook_is_rejected(self):
        manifest = Path("plugins/alpha/dm-plugin.toml")
        manifest.write_text(manifest_text("alpha", hooks={"pre_install": "hooks/missing.sh"}))
        self.repo.binaries(self.target)
        with self.assertRaises(SystemExit) as raised:
            run_quietly(release.package)
        self.assertIn("hooks/missing.sh", str(raised.exception))


if __name__ == "__main__":
    unittest.main(verbosity=2)
