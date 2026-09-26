use dameng_cli::self_update::{normalize_tag, validate_repository, verify_checksum};
use dameng_cli::{
    CONFIG_FILE, Config, Manifest, PluginStore, github_repository, prebuilt_target_label_for,
    progress_bar_for, release_tag_candidates, versions_differ,
};
use sha2::{Digest, Sha256};

#[test]
fn update_helpers_validate_repository_and_versions() {
    assert!(validate_repository("guangl/dameng-cli").is_ok());
    assert!(validate_repository("https://example.com/x").is_err());
    assert_eq!(normalize_tag("0.2.0").unwrap(), "v0.2.0");
    assert_eq!(normalize_tag("v0.2.0").unwrap(), "v0.2.0");
}

#[test]
fn update_checksum_verification_rejects_tampering() {
    let digest = format!("{:x}  archive\n", Sha256::digest(b"archive"));
    assert!(verify_checksum(b"archive", digest.as_bytes()).is_ok());
    assert!(verify_checksum(b"changed", digest.as_bytes()).is_err());
}

fn base_manifest() -> String {
    r#"
name = "demo"
version = "0.1.0"
description = "Demo plugin"
api_version = 1
"#
    .to_string()
}

#[test]
fn from_toml_accepts_satisfied_min_host_version() {
    let text = format!(
        r#"{}
min_host_version = "0.1.0"
"#,
        base_manifest()
    );
    let manifest = Manifest::from_toml(&text).unwrap();
    assert_eq!(manifest.min_host_version.as_deref(), Some("0.1.0"));
}

#[test]
fn from_toml_rejects_too_new_min_host_version() {
    let text = format!(
        r#"{}
min_host_version = "999.0.0"
"#,
        base_manifest()
    );
    assert!(Manifest::from_toml(&text).is_err());
}

#[test]
fn requests_consent_from_none_requires_declared_requests() {
    let empty = Manifest::from_toml(&base_manifest()).unwrap();
    assert!(!empty.requests_consent_from(None));

    let requesting = Manifest::from_toml(&format!(
        r#"{}
permissions = ["network"]
environment = ["DM_TOKEN"]
"#,
        base_manifest()
    ))
    .unwrap();
    assert!(requesting.requests_consent_from(None));
}

#[test]
fn requests_consent_compares_previous_declarations() {
    let previous = Manifest::from_toml(&format!(
        r#"{}
permissions = ["filesystem"]
environment = ["DM_HOME"]
"#,
        base_manifest()
    ))
    .unwrap();

    let same = Manifest::from_toml(&format!(
        r#"{}
permissions = ["filesystem"]
environment = ["DM_HOME"]
"#,
        base_manifest()
    ))
    .unwrap();
    assert!(!same.requests_consent_from(Some(&previous)));

    let new_permission = Manifest::from_toml(&format!(
        r#"{}
permissions = ["network"]
"#,
        base_manifest()
    ))
    .unwrap();
    assert!(new_permission.requests_consent_from(Some(&previous)));

    let new_environment = Manifest::from_toml(&format!(
        r#"{}
environment = ["DM_SECRET"]
"#,
        base_manifest()
    ))
    .unwrap();
    assert!(new_environment.requests_consent_from(Some(&previous)));
}

#[test]
fn progress_bar_for_supports_terminal_and_hidden() {
    assert!(progress_bar_for(8, false).is_hidden());
    drop(progress_bar_for(8, true));
}

#[test]
fn github_repository_parses_valid_sources() {
    assert_eq!(
        github_repository("https://github.com/guangl/dameng-cli"),
        Some(("guangl", "dameng-cli"))
    );
    assert_eq!(
        github_repository("https://github.com/guangl/dameng-cli.git"),
        Some(("guangl", "dameng-cli"))
    );
    assert_eq!(github_repository("https://example.com/a/b"), None);
    assert_eq!(github_repository("https://github.com/a/b/c"), None);
}

#[test]
fn prebuilt_target_label_maps_all_targets() {
    assert_eq!(
        prebuilt_target_label_for("aarch64-apple-darwin"),
        Some("aarch64-macos")
    );
    assert_eq!(
        prebuilt_target_label_for("x86_64-apple-darwin"),
        Some("x86_64-macos")
    );
    assert_eq!(
        prebuilt_target_label_for("x86_64-unknown-linux-gnu"),
        Some("x86_64-linux")
    );
    assert_eq!(
        prebuilt_target_label_for("x86_64-unknown-linux-musl"),
        Some("x86_64-linux")
    );
    assert_eq!(
        prebuilt_target_label_for("aarch64-unknown-linux-gnu"),
        Some("aarch64-linux")
    );
    assert_eq!(
        prebuilt_target_label_for("aarch64-unknown-linux-musl"),
        Some("aarch64-linux")
    );
    assert_eq!(
        prebuilt_target_label_for("x86_64-pc-windows-msvc"),
        Some("x86_64-windows")
    );
    assert_eq!(prebuilt_target_label_for("unknown-target"), None);
}

#[test]
fn release_tag_candidates_prefers_revision() {
    let manifest = Manifest::from_toml(&base_manifest()).unwrap();
    assert_eq!(
        release_tag_candidates(&manifest, Some("v1.2.3")),
        vec!["v1.2.3", "v0.1.0"]
    );
    assert_eq!(
        release_tag_candidates(&manifest, Some("1.2.3")),
        vec!["v1.2.3", "v0.1.0"]
    );
    assert_eq!(release_tag_candidates(&manifest, None), vec!["v0.1.0"]);
}

#[test]
fn versions_differ_falls_back_to_equality() {
    assert!(versions_differ("0.1.0", "0.2.0"));
    assert!(!versions_differ("0.2.0", "0.1.0"));
    assert!(versions_differ("abc", "def"));
    assert!(!versions_differ("abc", "abc"));
}

#[test]
fn load_reports_missing_plugin() {
    let temp = tempfile::tempdir().unwrap();
    let store = PluginStore::new(temp.path());
    let error = store.load("missing").unwrap_err();
    assert!(error.to_string().contains("not installed"), "{error:#}");
}
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

#[test]
fn store_applies_the_configuration_settings() {
    let temp = tempfile::tempdir().unwrap();
    let forced = PluginStore::new(temp.path())
        .with_progress(Some(true))
        .with_plugin_environment(vec!["DM_TEST_VALUE".to_owned()]);
    assert!(forced.progress_enabled());
    assert!(
        !PluginStore::new(temp.path())
            .with_progress(Some(false))
            .progress_enabled()
    );

    // Without an explicit setting the terminal decides, as before.
    use std::io::IsTerminal;
    assert_eq!(
        PluginStore::new(temp.path()).progress_enabled(),
        std::io::stderr().is_terminal()
    );
}
