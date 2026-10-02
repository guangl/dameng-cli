"""Regression checks for the release glibc symbol gate."""
import unittest
from pathlib import Path
from unittest.mock import patch
import check_glibc


class GlibcTests(unittest.TestCase):
    def test_version_order_is_numeric(self):
        versions = check_glibc.required_versions("Name: GLIBC_2.9 Name: GLIBC_2.28")
        self.assertEqual(max(versions), (2, 28))

    def test_rejects_private_abi_and_missing_requirements(self):
        for output in ["Name: GLIBC_PRIVATE", "Name: GCC_3.0"]:
            with self.assertRaises(ValueError):
                check_glibc.required_versions(output)

    def test_gate_accepts_baseline_and_rejects_newer_symbols(self):
        for version, accepted in [("2.17", True), ("2.28", True), ("2.34", False)]:
            with patch("check_glibc.subprocess.run") as run:
                run.return_value.stdout = f"Name: GLIBC_{version}"
                if accepted:
                    check_glibc.check_binary(Path("dm"))
                else:
                    with self.assertRaises(ValueError):
                        check_glibc.check_binary(Path("dm"))

    def test_all_bundled_binaries_are_checked(self):
        paths = check_glibc.binaries("x86_64-unknown-linux-gnu")
        self.assertEqual({p.name for p in paths}, {"dm", "dm-db", "dm-ssh"})
        with self.assertRaises(ValueError):
            check_glibc.binaries("x86_64-unknown-linux-musl")


if __name__ == "__main__":
    unittest.main()
