use dm_plugin_support::{config::*, diagnostics::DiagnosticReport};
#[test]
fn config_initialization_never_overwrites_and_reports_sources() {
    let temp = tempfile::TempDir::new().unwrap();
    let path = temp.path().join("config/config.toml");
    initialize(&path, "[defaults]\n").unwrap();
    assert!(initialize(&path, "new").is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "[defaults]\n");
    let entries = vec![setting("port", 22, false), setting("user", "root", true)];
    assert_eq!(entries[0].source, "default");
    assert_eq!(entries[1].source, "config");
    show(&path, entries, true).unwrap();
    show(&path, vec![setting("port", 22, false)], false).unwrap();
}
#[test]
fn diagnostics_check_tools_and_keys_without_network_or_secret_access() {
    let mut report = DiagnosticReport::default();
    report.tool("rustc");
    report.tool("nonexistent-dm-tool-987");
    let temp = tempfile::TempDir::new().unwrap();
    report.key("missing", &temp.path().join("missing"));
    let key = temp.path().join("key");
    std::fs::write(&key, "private").unwrap();
    report.key("prod", &key);
    assert_eq!(report.issues.len(), 2);
    assert_eq!(report.print(true).unwrap(), 1);
    assert_eq!(report.print(false).unwrap(), 1);
    assert_eq!(DiagnosticReport::default().print(false).unwrap(), 0);
}
