use dameng_cli::support::config::{initialize, setting, show};

#[test]
fn config_initialization_never_overwrites_and_reports_sources() {
    let temp = tempfile::TempDir::new().unwrap();
    let path = temp.path().join("config/config.toml");
    initialize(&path, "[log]\n").unwrap();
    assert!(initialize(&path, "new").is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "[log]\n");
    let entries = vec![setting("port", 22, false), setting("user", "root", true)];
    assert_eq!(entries[0].source, "default");
    assert_eq!(entries[1].source, "config");
    show(&path, entries, true).unwrap();
    show(&path, vec![setting("port", 22, false)], false).unwrap();
}
