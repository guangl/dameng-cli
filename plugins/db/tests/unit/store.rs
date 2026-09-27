//! Tests for the connections.sqlite3 store.

use crate::common::*;
use dm_plugin_db::{
    connection_password, encrypt, find_connection, load_connections, open_database,
    remove_connection, upsert_connection, validate_name,
};
use std::fs;
use tempfile::TempDir;

#[test]
fn validate_name_accepts_and_rejects() {
    assert!(validate_name("prod-01").is_ok());
    assert!(validate_name("").is_err());
    assert!(validate_name("../bad").is_err());
    assert!(validate_name("bad name").is_err());
    assert!(validate_name(&"a".repeat(65)).is_err());
}

#[test]
fn connection_store_round_trip() {
    let temp = TempDir::new().unwrap();
    let context = context(&temp);
    let secret = encrypt(&context, b"SYSDBA@123").unwrap();
    let mut connection = sample("prod");
    connection.schema = Some("DMHR".to_owned());
    connection.secret = Some(secret);
    upsert_connection(&context, &connection).unwrap();

    let stored = load_connections(&context).unwrap();
    assert_eq!(stored, vec![connection.clone()]);
    assert_eq!(
        connection_password(&context, &stored[0]).unwrap(),
        "SYSDBA@123"
    );
    assert_eq!(find_connection(&context, "prod").unwrap().host, "10.0.0.8");

    // A second save replaces the row instead of adding one.
    let mut updated = connection.clone();
    updated.host = "10.0.0.9".to_owned();
    upsert_connection(&context, &updated).unwrap();
    assert_eq!(load_connections(&context).unwrap().len(), 1);
    assert_eq!(find_connection(&context, "prod").unwrap().host, "10.0.0.9");

    remove_connection(&context, "prod").unwrap();
    assert!(load_connections(&context).unwrap().is_empty());
}

#[test]
fn missing_and_corrupt_connections_are_reported() {
    let temp = TempDir::new().unwrap();
    let context = context(&temp);
    assert!(remove_connection(&context, "missing").is_err());
    let error = find_connection(&context, "missing").unwrap_err();
    assert!(
        format!("{error:#}").contains("is not configured"),
        "{error:#}"
    );

    let mut connection = sample("kept");
    upsert_connection(&context, &connection).unwrap();
    assert!(connection_password(&context, &connection).is_err());
    connection.secret = Some(encrypt(&context, b"pw").unwrap());
    assert_eq!(connection_password(&context, &connection).unwrap(), "pw");

    fs::create_dir_all(&context.data_dir).unwrap();
    let database = context.data_dir.join("connections.sqlite3");
    fs::remove_file(&database).unwrap();
    let sqlite = rusqlite::Connection::open(&database).unwrap();
    sqlite
        .execute_batch("CREATE TABLE connections (name TEXT PRIMARY KEY)")
        .unwrap();
    drop(sqlite);
    assert!(load_connections(&context).is_err());
    assert!(upsert_connection(&context, &sample("bad")).is_err());
}

#[cfg(unix)]
#[test]
fn open_database_reports_readonly_store_error() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempDir::new().unwrap();
    let context = context(&temp);
    fs::create_dir_all(&context.data_dir).unwrap();
    let database = context.data_dir.join("connections.sqlite3");
    drop(rusqlite::Connection::open(&database).unwrap());
    fs::set_permissions(&database, fs::Permissions::from_mode(0o444)).unwrap();
    assert!(open_database(&context).is_err());
    fs::set_permissions(&database, fs::Permissions::from_mode(0o644)).unwrap();
}
