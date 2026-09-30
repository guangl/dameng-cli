"""Run generated adapters in real shells against isolated host/plugin stores."""
import argparse
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent.parent


def run(args, env, cwd):
    result = subprocess.run(args, env=env, cwd=cwd, text=True, capture_output=True, timeout=30)
    if result.returncode:
        raise AssertionError(f"{args[0]} failed:\n{result.stdout}\n{result.stderr}")
    return result.stdout


def shell_candidates(shell, script, words, env, cwd):
    source = shlex.quote(str(script))
    joined = " ".join(shlex.quote(word) for word in words)
    line = "dm " + " ".join(shlex.quote(word) for word in words[:-1])
    line += (" " if len(words) > 1 else "") + words[-1]
    if shell == "bash":
        code = f"source {source}; COMP_WORDS=(dm {joined}); COMP_CWORD={len(words)}; _dm; printf '%s\\n' \"${{COMPREPLY[@]}}\""
        output = run(["bash", "--noprofile", "--norc", "-c", code], env, cwd)
    elif shell == "zsh":
        code = f"autoload -Uz compinit; compinit -D -u; source {source}; compadd() {{ shift; print -rl -- \"$@\"; }}; words=(dm {joined}); CURRENT={len(words)+1}; _dm"
        output = run(["zsh", "-f", "-c", code], env, cwd)
    elif shell == "fish":
        output = run(["fish", "--no-config", "-c", f"source {source}; complete -C {shlex.quote(line)}"], env, cwd)
    elif shell == "powershell":
        environment = dict(env, DM_COMPLETION_LINE=line, DM_COMPLETION_SCRIPT=str(script))
        code = ". $env:DM_COMPLETION_SCRIPT; (TabExpansion2 $env:DM_COMPLETION_LINE $env:DM_COMPLETION_LINE.Length).CompletionMatches | ForEach-Object { $_.ListItemText }"
        output = run([shutil.which("pwsh") or "powershell", "-NoProfile", "-Command", code], environment, cwd)
    else:
        code = script.read_text() + "\n$edit:completion:arg-completer[dm] dm " + joined
        output = run(["elvish", "-c", "use edit\n" + code], env, cwd)
    return {entry.split("\t")[0] for entry in output.splitlines() if entry}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--require", default="bash,zsh" if os.name != "nt" else "powershell")
    options = parser.parse_args()
    executables = {"bash": "bash", "zsh": "zsh", "fish": "fish", "elvish": "elvish", "powershell": "pwsh"}
    available = [shell for shell, executable in executables.items() if shutil.which(executable)]
    required = set(filter(None, options.require.split(",")))
    assert required.issubset(available), f"Missing shells: {required - set(available)}"
    suffix = ".exe" if os.name == "nt" else ""
    binary_directory = ROOT / "target/debug"
    dm = str(binary_directory / f"dm{suffix}")
    with tempfile.TemporaryDirectory(prefix="dm-shells-") as directory:
        root = Path(directory)
        env = dict(os.environ, DM_PLUGIN_HOME=str(root / "home"), DM_LOG="off", HOME=str(root), ZDOTDIR=str(root))
        env["PATH"] = str(binary_directory) + os.pathsep + env["PATH"]
        for plugin in ["ssh", "db"]:
            package = root / plugin
            package.mkdir()
            shutil.copyfile(ROOT / f"plugins/{plugin}/dm-plugin.toml", package / "dm-plugin.toml")
            shutil.copyfile(binary_directory / f"dm-{plugin}{suffix}", package / f"dm-{plugin}{suffix}")
            run([dm, "install", str(package)], env, root)
        for name in ["prod", "stage"]:
            run([dm, "ssh", "add", name, "--host", "example.invalid", "--username", "root", "--password", "never-echo-this"], env, root)
        run([dm, "db", "add", "prod", "--host", "example.invalid", "--password", "never-echo-this"], env, root)
        key = root / "private key"
        key.write_text("fixture")
        cases = [
            ([""], {"ssh", "db", "config"}),
            (["ssh", ""], {"connect", "edit", "doctor"}),
            (["ssh", "connect", "pr"], {"prod"}),
            (["ssh", "connect", ""], {"prod", "stage"}),
            (["db", "edit", "prod", "--"], {"--port", "--clear-schema"}),
            (["ssh", "add", "new", "--key", str(root / "private")], {str(key)}),
            (["ssh", "edit", "prod", "--password", "pw", "--"], {"--host"}),
        ]
        for shell in available:
            script = root / f"completion.{shell}"
            script.write_text(run([dm, "completions", shell], env, root))
            for words, expected in cases:
                candidates = shell_candidates(shell, script, words, env, root)
                assert expected.issubset(candidates), (shell, words, expected, candidates)
                assert "never-echo-this" not in candidates
                if "--password" in words:
                    assert "--key" not in candidates and "--passphrase" not in candidates, (shell, words, candidates)
            print(f"{shell}: {len(cases)} runtime completion scenarios passed")


if __name__ == "__main__":
    main()
