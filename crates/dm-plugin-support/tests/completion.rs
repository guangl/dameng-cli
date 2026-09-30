use clap::{Arg, ArgAction, Command};
use dm_plugin_support::completion::candidates;
fn command() -> Command {
    Command::new("dm")
        .subcommand(
            Command::new("ssh").subcommand(
                Command::new("connect")
                    .visible_alias("ssh")
                    .arg(Arg::new("name").index(1))
                    .arg(Arg::new("key").long("key").short('i'))
                    .arg(Arg::new("file").long("file"))
                    .arg(Arg::new("yes").long("yes").action(ArgAction::SetTrue))
                    .arg(Arg::new("mode").long("mode").value_parser(["one", "two"]))
                    .arg(Arg::new("hidden").long("hidden").hide(true)),
            ),
        )
        .subcommand(Command::new("secret").hide(true))
}
fn complete(words: &[&str]) -> Vec<String> {
    candidates(
        command(),
        &words.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        &["prod".into(), "stage".into()],
    )
}
#[test]
fn completion_tracks_subcommands_aliases_arguments_and_pending_values() {
    assert!(complete(&[""]).contains(&"ssh".into()));
    assert!(!complete(&[""]).contains(&"secret".into()));
    assert!(complete(&["ssh", ""]).contains(&"connect".into()));
    assert!(complete(&["ssh", ""]).contains(&"ssh".into()));
    assert_eq!(complete(&["ssh", "connect", "pr"]), ["prod"]);
    assert!(complete(&["ssh", "connect", "--"]).contains(&"--key".into()));
    assert!(!complete(&["ssh", "connect", "--"]).contains(&"--hidden".into()));
    assert_eq!(complete(&["ssh", "connect", "--mode", "t"]), ["two"]);
    assert_eq!(complete(&["ssh", "connect", "--mode=o"]), ["--mode=one"]);
    assert!(complete(&["ssh", "connect", "--key", "value", "pr"]).contains(&"prod".into()));
    assert!(complete(&["ssh", "ssh", "pr"]).contains(&"prod".into()));
    assert!(!complete(&["ssh", "connect", "--yes", "--"]).contains(&"--yes".into()));
    assert_eq!(complete(&["ssh", "connect", "--", "pr"]), ["prod"]);
    assert!(
        complete(&["ssh", "connect", "prod", ""])
            .iter()
            .all(|s| s.starts_with('-'))
    );
    assert!(complete(&["ssh", "connect", "--unknown=x"]).is_empty());
    assert!(complete(&[]).contains(&"ssh".into()));
}
#[test]
fn completion_preserves_spaces_in_paths_and_does_not_offer_secrets() {
    let temp = tempfile::TempDir::new().unwrap();
    let prefix = format!("{}/", temp.path().display());
    std::fs::write(temp.path().join("my key"), "secret").unwrap();
    std::fs::write(temp.path().join(".hidden"), "secret").unwrap();
    std::fs::create_dir(temp.path().join("folder")).unwrap();
    let values = complete(&["ssh", "connect", "--key", &prefix]);
    assert!(values.contains(&format!("{prefix}my key")));
    assert!(values.contains(&format!("{prefix}folder/")));
    assert!(!values.iter().any(|v| v.contains(".hidden")));
    assert!(
        complete(&["ssh", "connect", "-i", &format!("{prefix}m")])
            .contains(&format!("{prefix}my key"))
    );
    assert!(complete(&["ssh", "connect", "--key", "/missing-parent-dm-test/"]).is_empty());
}
