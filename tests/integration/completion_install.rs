//! End-to-end checks for installing shell completion.
use crate::common::*;
use tempfile::TempDir;

#[test]
fn completion_install_writes_the_script_the_shell_sources() {
    let temp = TempDir::new().unwrap();
    let installed = temp.path().join("completions");
    let output = ok(dm(temp.path())
        .args([
            "completions",
            "zsh",
            "--install",
            "--dir",
            installed.to_str().unwrap(),
        ])
        .output()
        .unwrap());
    assert!(output.contains("已安装 Zsh 补全"), "{output}");
    assert!(output.contains("fpath"), "{output}");
    assert_eq!(
        std::fs::read_to_string(installed.join("_dm")).unwrap(),
        ok(dm(temp.path())
            .args(["completions", "zsh"])
            .output()
            .unwrap())
    );

    let bash = temp.path().join("nested/bash");
    ok(dm(temp.path())
        .args([
            "completions",
            "bash",
            "--install",
            "--dir",
            bash.to_str().unwrap(),
        ])
        .output()
        .unwrap());
    assert_eq!(
        std::fs::read_to_string(bash.join("dm")).unwrap(),
        ok(dm(temp.path())
            .args(["completions", "bash"])
            .output()
            .unwrap())
    );
}

#[test]
fn completion_install_requires_the_flag_and_a_known_shell() {
    let temp = TempDir::new().unwrap();
    let output = dm(temp.path())
        .args(["completions", "bash", "--dir", "/tmp/dm-completion-missing"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--install"));

    let default = dm(temp.path())
        .args(["completions", "zsh", "--install"])
        .env("HOME", temp.path())
        .env_remove("XDG_DATA_HOME")
        .output()
        .unwrap();
    assert!(ok(default).contains("site-functions"));
    assert!(
        temp.path()
            .join(".local/share/zsh/site-functions/_dm")
            .is_file()
    );
}
