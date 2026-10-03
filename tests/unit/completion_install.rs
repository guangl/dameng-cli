use dameng_cli::cli::{Shell, activation_hint, completion_dir, install, script};
use std::{ffi::OsStr, path::Path};

#[test]
fn completion_install_writes_the_file_the_shell_loads() {
    let temp = tempfile::TempDir::new().unwrap();
    let bash = install(Shell::Bash, Some(temp.path())).unwrap();
    assert_eq!(bash.file_name().unwrap(), "dm");
    assert_eq!(std::fs::read_to_string(&bash).unwrap(), script(Shell::Bash));
    let nested = install(Shell::Zsh, Some(&temp.path().join("a/b"))).unwrap();
    assert_eq!(nested.file_name().unwrap(), "_dm");
    assert_eq!(
        std::fs::read_to_string(&nested).unwrap(),
        script(Shell::Zsh)
    );
    assert!(activation_hint(Shell::Bash, &bash).contains(&temp.path().display().to_string()));
    assert!(activation_hint(Shell::Zsh, &nested).contains("fpath"));
}

#[test]
fn completion_directory_follows_xdg_then_falls_back_to_home() {
    let xdg = OsStr::new("/tmp/example-xdg");
    assert_eq!(
        completion_dir(Shell::Bash, Some(xdg), None).unwrap(),
        Path::new("/tmp/example-xdg/bash-completion/completions")
    );
    assert_eq!(
        completion_dir(Shell::Zsh, Some(xdg), None).unwrap(),
        Path::new("/tmp/example-xdg/zsh/site-functions")
    );
    let home = OsStr::new("/home/example");
    assert_eq!(
        completion_dir(Shell::Zsh, None, Some(home)).unwrap(),
        Path::new("/home/example/.local/share/zsh/site-functions")
    );
    assert_eq!(
        completion_dir(Shell::Bash, Some(OsStr::new("")), Some(home)).unwrap(),
        Path::new("/home/example/.local/share/bash-completion/completions")
    );
    assert!(completion_dir(Shell::Bash, None, None).is_none());
    assert!(completion_dir(Shell::Zsh, Some(OsStr::new("")), Some(OsStr::new(""))).is_none());
}
