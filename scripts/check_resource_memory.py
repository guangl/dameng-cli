#!/usr/bin/env python3
"""Check that archive verification memory does not scale with archive size."""
import hashlib
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
PROBE = r'''
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let archive = std::path::Path::new(&args[2]);
    let checksum = std::path::Path::new(&args[3]);
    if args[1] == "stream" {
        dameng_cli::self_update::verify_checksum_file(archive, checksum).unwrap();
    } else {
        dameng_cli::self_update::verify_checksum(
            &std::fs::read(archive).unwrap(), &std::fs::read(checksum).unwrap()).unwrap();
    }
}
'''


def main():
    if sys.platform not in ("linux", "darwin"):
        raise SystemExit("Resource memory check requires Linux or macOS /usr/bin/time")
    subprocess.run(["cargo", "build", "--lib", "--locked", "--quiet"], cwd=ROOT, check=True)
    target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
    if not target.is_absolute():
        target = ROOT / target
    dependencies = target / "debug/deps"
    library = max(dependencies.glob("libdameng_cli-*.rlib"), key=lambda path: path.stat().st_mtime)
    with tempfile.TemporaryDirectory(prefix="dm-memory-") as temporary:
        folder = Path(temporary)
        archive, checksum = folder / "archive", folder / "checksum"
        digest = hashlib.sha256()
        block = b"\x55" * (1024 * 1024)
        with archive.open("wb") as output:
            for _ in range(128):
                output.write(block)
                digest.update(block)
        checksum.write_text(digest.hexdigest())
        source, executable = folder / "probe.rs", folder / "probe"
        source.write_text(PROBE)
        subprocess.run(["rustc", "--edition=2024", str(source), "--extern",
                        f"dameng_cli={library}", "-L", f"dependency={dependencies}",
                        "-o", str(executable)], check=True)
        peaks = {}
        for mode in ("whole", "stream"):
            flag = "-l" if sys.platform == "darwin" else "-v"
            result = subprocess.run(["/usr/bin/time", flag, str(executable), mode,
                                     str(archive), str(checksum)], check=True,
                                    capture_output=True, text=True)
            if sys.platform == "darwin":
                match = re.search(r"(\d+)\s+maximum resident set size", result.stderr)
                multiplier = 1
            else:
                match = re.search(r"Maximum resident set size \(kbytes\):\s*(\d+)", result.stderr)
                multiplier = 1024
            if not match:
                raise SystemExit(f"Cannot parse memory statistics: {result.stderr}")
            peaks[mode] = int(match.group(1)) * multiplier
            print(f"{mode}: peak RSS {peaks[mode] / (1024 * 1024):.2f} MiB (128 MiB archive)")
        if peaks["stream"] > 32 * 1024 * 1024:
            raise SystemExit("Streaming checksum exceeded the 32 MiB probe memory budget")
        if peaks["whole"] - peaks["stream"] < 64 * 1024 * 1024:
            raise SystemExit("Checksum probe did not demonstrate bounded memory")


if __name__ == "__main__":
    main()
