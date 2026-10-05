"""Offline installer tests for independent sources and old release fallback."""
import contextlib
import io
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
import release

ROOT = Path(__file__).resolve().parent.parent
TARGET = "aarch64-apple-darwin"
TAG = "v0.4.1"


class InstallerTests(unittest.TestCase):
    def install(self, mode, plugins=None):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            previous = Path.cwd()
            try:
                os.chdir(root)
                Path("Cargo.toml").write_text('[package]\nversion = "0.4.1"\n')
                Path("README.md").write_text("fixture")
                Path("LICENSE").write_text("MIT")
                binaries = root / "target" / TARGET / "release"
                binaries.mkdir(parents=True)
                host = binaries / "dm"
                host.write_text('''#!/bin/sh
if [ "$*" = 'install --help' ]; then
  echo '--release-source'
elif [ "$*" = 'completions --help' ]; then
  echo '--install'
else
  printf '%s\\n' "$*" >> "$DM_TEST_LOG"
fi
''')
                host.chmod(0o755)
                plugin = root / "plugins/db"
                plugin.mkdir(parents=True)
                (plugin / "Cargo.toml").write_text('[package]\nversion = "0.2.0"\nrepository = "https://github.com/guangl/dm-plugin-db"\n')
                (plugin / "dm-plugin.toml").write_text('name = "db"\nversion = "0.2.0"\napi_version = 1\n')
                (binaries / "dm-db").write_bytes(b"fixture")
                with patch.dict(os.environ, RELEASE_TAG=TAG, RELEASE_TARGET=TARGET), contextlib.redirect_stdout(io.StringIO()):
                    release.package()
                sources = root / "dist" / f"dm-plugin-sources-{TAG}-{TARGET}.txt"
                if mode == "legacy":
                    sources.unlink()
                    sources.with_name(sources.name + ".sha256").unlink()
                elif mode == "tampered":
                    sources.write_text("db attacker/plugin v9.9.9\n")
                elif mode == "missing-checksum":
                    sources.with_name(sources.name + ".sha256").unlink()
                elif mode == "missing-plugin":
                    sources.write_text("ssh guangl/dm-plugin-ssh v0.2.0\n")
                    release._write_checksum(sources)
                tools = root / "tools"
                tools.mkdir()
                curl = tools / "curl"
                curl.write_text(f'#!{sys.executable}\n' + '''import os
from pathlib import Path
import shutil
import sys
args = sys.argv[1:]
asset = Path(args[-1]).name
source = Path(os.environ["DM_TEST_ASSETS"]) / asset
destination = Path(args[args.index("-o") + 1])
if source.exists():
    shutil.copy2(source, destination)
    print("200", end="")
else:
    print("404", end="")
''')
                curl.chmod(0o755)
                log = root / "installed.log"
                env = dict(os.environ, PATH=str(tools) + os.pathsep + os.environ["PATH"],
                           DM_INSTALL_DIR=str(root / "bin"), DM_INSTALL_TARGET=TARGET,
                           DM_TEST_LOG=str(log), DM_TEST_ASSETS=str(root / "dist"))
                env.pop("DM_INSTALL_PLUGINS", None)
                if plugins is not None:
                    env["DM_INSTALL_PLUGINS"] = plugins
                result = subprocess.run(["sh", str(ROOT / "scripts/install.sh"), TAG],
                                        env=env, capture_output=True, text=True, timeout=30)
                return result, log.read_text() if log.exists() else ""
            finally:
                os.chdir(previous)

    def test_installed_bundle_tracks_independent_repository_and_plugin_tag(self):
        result, log = self.install("independent")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("--release-source guangl/dm-plugin-db --release-tag v0.2.0", log)
        # The installer also installs shell completion for both supported shells.
        self.assertIn("completions bash --install", log)
        self.assertIn("completions zsh --install", log)

    def test_explicit_selection_deduplicates(self):
        result, log = self.install("independent", "db,db")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(log.count("--release-source"), 1)

    def test_empty_selection_skips_plugins_and_sources(self):
        result, log = self.install("tampered", "")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertNotIn("--release-source", log)
        self.assertIn("completions bash --install", log)

    def test_unknown_or_unsafe_selection_fails(self):
        for plugins in ("ssh", "../db", "*", "db,unknown"):
            with self.subTest(plugins=plugins):
                result, log = self.install("independent", plugins)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(log, "")

    def test_local_selection_controls_build_and_install(self):
        for selection, expected in (("db,db", ["db"]), ("", []), ("ssh db", ["ssh", "db"]), ("sqllog2db", []), ("unknown", None)):
            with self.subTest(selection=selection), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                (root / "scripts").mkdir()
                shutil.copy2(ROOT / "scripts/install-local.sh", root / "scripts/install-local.sh")
                for component in ("crates/dm-plugin-sdk", "plugins/db", "plugins/ssh"):
                    path = root / component
                    path.mkdir(parents=True)
                    (path / "Cargo.toml").touch()
                binaries = root / "target/release"
                binaries.mkdir(parents=True)
                host = binaries / "dm"
                host.write_text('#!/bin/sh\nprintf "%s\\n" "$*" >> "$DM_TEST_LOG"\n')
                host.chmod(0o755)
                for plugin in ("ssh", "db"):
                    shutil.copy2(host, binaries / f"dm-{plugin}")
                tools = root / "tools"
                tools.mkdir()
                cargo = tools / "cargo"
                cargo.write_text('#!/bin/sh\nprintf "%s\\n" "$*" >> "$DM_TEST_LOG"\n')
                cargo.chmod(0o755)
                log = root / "log"
                env = dict(os.environ, PATH=str(tools) + os.pathsep + os.environ["PATH"],
                           DM_INSTALL_PLUGINS=selection, DM_TEST_LOG=str(log), DM_INSTALL_DIR=str(root / "bin"))
                result = subprocess.run(["sh", str(root / "scripts/install-local.sh")], env=env,
                                        capture_output=True, text=True, timeout=30)
                if expected is None:
                    self.assertNotEqual(result.returncode, 0)
                    self.assertFalse(log.exists())
                    continue
                self.assertEqual(result.returncode, 0, result.stderr)
                lines = log.read_text().splitlines()
                if selection == "sqllog2db":
                    self.assertIn("install https://github.com/guangl/dm-database-sqllog2db.git --rev v3.0.2 --replace", lines)
                for plugin in ("ssh", "db"):
                    self.assertEqual(f"-p dm-plugin-{plugin}" in lines[0], plugin in expected)
                    installs = [line for line in lines if line.startswith("install ") and f"plugins/{plugin} " in line]
                    self.assertEqual(len(installs), int(plugin in expected))

    def test_external_sqllog2db_selection(self):
        result, log = self.install("independent", "db,sqllog2db")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("--release-source guangl/dm-plugin-db", log)
        self.assertIn("install https://github.com/guangl/dm-database-sqllog2db.git --rev v3.0.2 --replace", log)

    def test_old_release_tracks_host_repository_and_host_tag(self):
        result, log = self.install("legacy")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("--release-source guangl/dameng-cli --release-tag v0.4.1", log)

    def test_invalid_sources_stop_before_plugin_installation(self):
        for mode in ("tampered", "missing-checksum", "missing-plugin"):
            with self.subTest(mode=mode):
                result, log = self.install(mode)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(log, "")


if __name__ == "__main__":
    unittest.main()
