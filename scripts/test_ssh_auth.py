"""Authenticate with a saved encrypted key against an isolated loopback sshd (Linux)."""
import getpass
import os
from pathlib import Path
import shutil
import socket
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parent.parent


def run(args, **kwargs):
    result = subprocess.run(args, text=True, capture_output=True, timeout=20, **kwargs)
    assert result.returncode == 0, f"{args[0]} failed: {result.stdout}\n{result.stderr}"
    return result.stdout


def main():
    sshd = shutil.which("sshd") or "/usr/sbin/sshd"
    assert Path(sshd).is_file(), "sshd is required for this compatibility test"
    privilege = [] if os.geteuid() == 0 else ["sudo", "-n"]
    run(privilege + ["mkdir", "-p", "/run/sshd"])
    with tempfile.TemporaryDirectory(prefix="dm-ssh-auth-") as directory:
        root = Path(directory)
        client_key = root / "encrypted key"
        host_key = root / "host_key"
        phrase = "regression-fixture-passphrase"
        for key, secret in [(host_key, ""), (client_key, phrase)]:
            run(["ssh-keygen", "-q", "-t", "ed25519", "-N", secret, "-f", str(key)])
        # sshd's unprivileged authentication child must be able to read the public key.
        root.chmod(0o755)
        public_key = Path(str(client_key) + ".pub")
        public_key.chmod(0o644)
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            port = listener.getsockname()[1]
        pid_file = root / "sshd.pid"
        config = root / "sshd_config"
        config.write_text(
            f'Port {port}\nListenAddress 127.0.0.1\nHostKey "{host_key}"\n'
            f'PidFile "{pid_file}"\nAuthorizedKeysFile "{public_key}"\n'
            f"AllowUsers {getpass.getuser()}\nStrictModes no\nUsePAM yes\n"
            "PasswordAuthentication no\nKbdInteractiveAuthentication no\nLogLevel ERROR\n"
        )
        run(privilege + [sshd, "-t", "-f", str(config)])
        log = root / "sshd.log"
        with log.open("w") as server_log:
            server = subprocess.Popen(
                privilege + [sshd, "-D", "-e", "-f", str(config)],
                stdout=server_log, stderr=server_log,
            )
            try:
                for _ in range(100):
                    assert server.poll() is None, log.read_text()
                    try:
                        with socket.create_connection(("127.0.0.1", port), timeout=0.1):
                            break
                    except OSError:
                        time.sleep(0.05)
                else:
                    raise AssertionError("loopback sshd did not start")
                env = dict(os.environ, DM_PLUGIN_HOME=str(root / "home"), DM_LOG="off")
                binary = str(ROOT / "target/debug/dm")
                package = root / "package"
                package.mkdir()
                shutil.copy2(ROOT / "target/debug/dm-ssh", package / "dm-ssh")
                shutil.copy2(ROOT / "plugins/ssh/dm-plugin.toml", package / "dm-plugin.toml")
                run([binary, "install", str(package)], env=env)
                run([binary, "ssh", "add", "keyed", "--host", "127.0.0.1", "--port", str(port),
                     "--username", getpass.getuser(), "--key", str(client_key),
                     "--passphrase", phrase], env=env)
                assert "测试成功" in run([binary, "ssh", "test", "keyed"], env=env)
                login = run([binary, "ssh", "connect", "keyed"], env=env,
                            input="printf 'saved-key-login-ok\\n'\nexit\n")
                assert "saved-key-login-ok" in login
                run([binary, "ssh", "edit", "keyed", "--passphrase", "wrong-fixture"], env=env)
                failure = subprocess.run([binary, "ssh", "test", "keyed"], env=env,
                                         text=True, capture_output=True, timeout=20)
                assert failure.returncode != 0, "incorrect saved passphrase must fail"
                assert phrase not in failure.stdout + failure.stderr
                print("encrypted key: probe, login and wrong-passphrase scenarios passed")
            finally:
                if pid_file.exists():
                    run(privilege + ["kill", pid_file.read_text().strip()])
                server.wait(timeout=10)


if __name__ == "__main__":
    main()
