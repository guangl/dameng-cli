//! `Config` parsing for the supported keys and the errors for the rest.

use dameng_cli::{CONFIG_FILE, Config};

#[test]
fn config_file_parses_supported_keys() {
    let config =
        Config::from_toml("[log]\nlevel = \"debug\"\n\n[update]\nrepository = \"owner/repo\"\n")
            .unwrap();
    assert_eq!(config.log.level.as_deref(), Some("debug"));
    assert_eq!(config.update.repository.as_deref(), Some("owner/repo"));

    // Whitespace is trimmed so stray padding cannot change behavior.
    let config = Config::from_toml("[log]\nlevel = '  dm=debug  '\n").unwrap();
    assert_eq!(config.log.level.as_deref(), Some("dm=debug"));
    assert_eq!(config.update.repository, None);
}
#[test]
fn config_file_rejects_unknown_tables_keys_and_empty_values() {
    // A key outside its table is rejected like any other unknown key.
    let error = Config::from_toml("loglevel = \"debug\"\n").unwrap_err();
    assert!(error.to_string().contains("unknown field"), "{error:#}");

    let error = Config::from_toml("[update]\nrepository = \"\"\n").unwrap_err();
    assert!(error.to_string().contains("must not be empty"), "{error:#}");

    let error = Config::from_toml("[logs]\nlevel = \"debug\"\n").unwrap_err();
    assert!(error.to_string().contains("unknown field"), "{error:#}");

    let error = Config::from_toml("[log]\nloglevel = \"debug\"\n").unwrap_err();
    assert!(error.to_string().contains("unknown field"), "{error:#}");
}
#[test]
fn config_file_is_optional() {
    let temp = tempfile::tempdir().unwrap();
    assert_eq!(Config::path_in(temp.path()), temp.path().join(CONFIG_FILE));
    assert_eq!(Config::load(temp.path()).unwrap(), Config::default());

    std::fs::write(Config::path_in(temp.path()), "[log]\nlevel = \"info\"\n").unwrap();
    assert_eq!(
        Config::load(temp.path()).unwrap().log.level.as_deref(),
        Some("info")
    );

    std::fs::write(Config::path_in(temp.path()), "[log]\nlevel = ;\n").unwrap();
    let error = Config::load(temp.path()).unwrap_err();
    assert!(format!("{error:#}").contains("config.toml"), "{error:#}");
}
#[test]
fn config_file_parses_the_optional_keys() {
    let config = Config::from_toml(
        r#"
[log]
level = "info"

[update]
target = "aarch64-apple-darwin"

[output]
progress = false

[plugin]
environment = ["DM_DATABASE_URL", " DM_DATABASE_URL ", "PGPASSWORD"]
"#,
    )
    .unwrap();
    assert_eq!(
        config.update.target.as_deref(),
        Some("aarch64-apple-darwin")
    );
    assert_eq!(config.output.progress, Some(false));
    // Entries are trimmed and de-duplicated while keeping their order.
    assert_eq!(
        config.plugin.environment,
        vec!["DM_DATABASE_URL".to_owned(), "PGPASSWORD".to_owned()]
    );
    assert_eq!(
        Config::from_toml("[output]\nprogress = true\n")
            .unwrap()
            .output
            .progress,
        Some(true)
    );
}
#[test]
fn config_file_rejects_invalid_switches_and_environment_names() {
    let error = Config::from_toml("[output]\nprogress = \"yes\"\n").unwrap_err();
    assert!(error.to_string().contains("invalid type"), "{error:#}");

    let error = Config::from_toml("[plugin]\nenvironment = \"DM_DATABASE_URL\"\n").unwrap_err();
    assert!(error.to_string().contains("invalid type"), "{error:#}");

    let error = Config::from_toml("[plugin]\nenvironment = [\"\"]\n").unwrap_err();
    assert!(error.to_string().contains("empty names"), "{error:#}");

    let error = Config::from_toml("[plugin]\nenvironment = [\"DB-URL\"]\n").unwrap_err();
    assert!(
        error
            .to_string()
            .contains("not a valid environment variable"),
        "{error:#}"
    );

    let error = Config::from_toml("[plugin]\nenvironment = [\"1LEADING\"]\n").unwrap_err();
    assert!(error.to_string().contains("not a valid"), "{error:#}");
}
