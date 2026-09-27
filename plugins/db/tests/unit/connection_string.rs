//! Tests for the ODBC connection string built for test/exec.

use crate::common::*;
use dm_plugin_db::build_connection_string;

#[test]
fn connection_string_covers_the_supported_fields() {
    let mut connection = sample("prod");
    assert_eq!(
        build_connection_string(&connection, "SYSDBA@123").unwrap(),
        "Driver={DM8 ODBC DRIVER};Server=10.0.0.8;Port=5236;UID=SYSDBA;PWD=SYSDBA@123;"
    );

    connection.schema = Some("DMHR".to_owned());
    assert_eq!(
        build_connection_string(&connection, "pw").unwrap(),
        "Driver={DM8 ODBC DRIVER};Server=10.0.0.8;Port=5236;UID=SYSDBA;PWD=pw;Schema=DMHR;"
    );

    // Separators and braces in the password are quoted.
    assert!(
        build_connection_string(&connection, "p;w}d")
            .unwrap()
            .contains("PWD={p;w}}d};")
    );
}

#[test]
fn connection_string_rejects_invalid_fields() {
    let mut connection = sample("prod");
    connection.host = "bad;host".to_owned();
    assert!(build_connection_string(&connection, "pw").is_err());

    let mut connection = sample("prod");
    connection.username = "bad{user}".to_owned();
    assert!(build_connection_string(&connection, "pw").is_err());

    let mut connection = sample("prod");
    connection.schema = Some("bad;schema".to_owned());
    assert!(build_connection_string(&connection, "pw").is_err());

    let mut connection = sample("prod");
    connection.driver = "  ".to_owned();
    assert!(build_connection_string(&connection, "pw").is_err());

    let mut connection = sample("prod");
    connection.host = "  ".to_owned();
    assert!(build_connection_string(&connection, "pw").is_err());

    let mut connection = sample("prod");
    connection.username = String::new();
    assert!(build_connection_string(&connection, "pw").is_err());
}
