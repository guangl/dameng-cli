//! The host command surface stays small and checking versions never applies updates.

use clap::{CommandFactory, Parser};
use dameng_cli::cli::{Cli, Command};

#[test]
fn default_update_is_a_version_check() {
    for args in [vec!["dm", "update"], vec!["dm", "update", "--json"]] {
        let cli = Cli::try_parse_from(args).unwrap();
        assert!(matches!(
            cli.command,
            Command::Update {
                name: None,
                all: false,
                ..
            }
        ));
    }
}

#[test]
fn update_json_cannot_be_combined_with_installing_updates() {
    for args in [
        ["dm", "update", "probe", "--json"],
        ["dm", "update", "--all", "--json"],
    ] {
        assert!(Cli::try_parse_from(args).is_err());
    }
}

#[test]
fn install_check_parses_with_its_alias_and_refuses_release_sources() {
    for args in [
        vec!["dm", "install", "./package", "--check"],
        vec!["dm", "install", "./package", "--dry-run"],
    ] {
        let cli = Cli::try_parse_from(args).unwrap();
        assert!(matches!(
            cli.command,
            Command::Install {
                check: true,
                replace: false,
                ..
            }
        ));
    }
    // The installer's persistent Release source records an installation, so it
    // cannot be combined with a check that installs nothing.
    assert!(
        Cli::try_parse_from([
            "dm",
            "install",
            "./package",
            "--check",
            "--release-source",
            "guangl/dameng-cli",
            "--release-tag",
            "v1.0.0",
        ])
        .is_err()
    );
}

#[test]
fn removed_commands_are_absent_from_help_and_no_longer_reserved() {
    let command = Cli::command();
    let names: Vec<_> = command
        .get_subcommands()
        .map(|subcommand| subcommand.get_name())
        .collect();
    assert!(!names.contains(&"outdated"));
    assert!(!names.contains(&"verify"));
    for name in ["outdated", "verify"] {
        let text = format!(
            "name = '{name}'\nversion = '1.0.0'\ndescription = 'test plugin'\napi_version = 1\n"
        );
        assert!(dameng_cli::Manifest::from_toml(&text).is_ok());
    }
}
