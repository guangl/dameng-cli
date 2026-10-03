use clap::CommandFactory;
use dameng_cli::cli::Cli;
use dameng_cli::support::completion::candidates;

fn complete(words: &[&str], names: &[&str]) -> Vec<String> {
    candidates(
        Cli::command(),
        &words.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        &names.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
    )
}

#[test]
fn completion_tracks_host_subcommands_flags_and_installed_names() {
    assert!(complete(&[], &[]).contains(&"install".into()));
    assert!(complete(&[""], &[]).contains(&"list".into()));
    assert_eq!(complete(&["ins"], &[]), ["install"]);
    assert!(complete(&["install", "--"], &[]).contains(&"--replace".into()));
    assert!(!complete(&["install", "--"], &[]).contains(&"--unknown".into()));
    assert!(complete(&["completions", ""], &[]).contains(&"bash".into()));
    assert_eq!(complete(&["completions", "b"], &[]), ["bash"]);
    assert_eq!(complete(&["info", "pr"], &["prod", "stage"]), ["prod"]);
    assert!(complete(&["update", ""], &["prod", "stage"]).contains(&"stage".into()));
    assert_eq!(
        complete(&["uninstall", "st"], &["prod", "stage"]),
        ["stage"]
    );
    assert!(complete(&["install", "--unknown=x"], &[]).is_empty());
    assert_eq!(complete(&["install", "--rev"], &[]), ["--rev"]);
}

#[test]
fn completion_covers_config_and_self_update_subcommands() {
    let config = complete(&["config", ""], &[]);
    assert!(config.contains(&"init".into()));
    assert!(config.contains(&"show".into()));
    assert!(config.contains(&"path".into()));
    assert!(complete(&["self-update", "--"], &[]).contains(&"--target".into()));
    assert!(complete(&["config", "show", "--"], &[]).contains(&"--json".into()));
}

#[test]
fn completion_offers_paths_without_hidden_files_or_secrets() {
    let temp = tempfile::TempDir::new().unwrap();
    let prefix = format!("{}/", temp.path().display());
    std::fs::write(temp.path().join("my package"), "fixture").unwrap();
    std::fs::write(temp.path().join(".hidden"), "fixture").unwrap();
    std::fs::create_dir(temp.path().join("folder")).unwrap();
    let values = complete(&["install", &prefix], &[]);
    assert!(values.contains(&format!("{prefix}my package")));
    assert!(values.contains(&format!("{prefix}folder/")));
    assert!(!values.iter().any(|value| value.contains(".hidden")));
    assert!(complete(&["install", "/missing-parent-dm-test/"], &[]).is_empty());
}
