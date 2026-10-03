//! A plugin must keep its tables in its own SQLite file, not in the host store.
use crate::common::*;
use dameng_cli::PluginStore;
use tempfile::TempDir;

fn table_exists(home: &std::path::Path, table: &str) -> i64 {
    let connection = rusqlite::Connection::open(home.join("store.sqlite3")).unwrap();
    connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [table],
            |row| row.get(0),
        )
        .unwrap()
}

#[test]
fn doctor_reports_tables_a_plugin_created_in_the_host_store() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let home = temp.path().join("home");
    let store = PluginStore::new(&home);
    store.install(source.to_str().unwrap()).unwrap();
    let connection = rusqlite::Connection::open(home.join("store.sqlite3")).unwrap();
    connection
        .execute("CREATE TABLE plugin_notes (name TEXT)", [])
        .unwrap();
    drop(connection);

    let report = store.doctor(false).unwrap();
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.contains("unexpected table in the host store: plugin_notes")),
        "{report:?}"
    );
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.contains("DM_PLUGIN_DATA_DIR")),
        "the issue must point at the plugin's own directory: {report:?}"
    );

    // Repair never drops data the host does not own, so the report stays.
    let repaired = store.doctor(true).unwrap();
    assert!(
        repaired
            .issues
            .iter()
            .any(|issue| issue.contains("plugin_notes")),
        "{repaired:?}"
    );
    assert_eq!(table_exists(&home, "plugin_notes"), 1);
    assert_eq!(table_exists(&home, "installed_plugins"), 1);
}
