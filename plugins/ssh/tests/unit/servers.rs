use std::fs;

use dm_plugin_ssh::{
    Server, decrypt, encrypt, load_servers, open_database, remove_server, upsert_server,
    validate_name,
};
use tempfile::TempDir;

use crate::common::*;

#[test]
fn validate_name_accepts_and_rejects() {
    assert!(validate_name("prod-01").is_ok());
    assert!(validate_name("").is_err());
    assert!(validate_name("../bad").is_err());
    assert!(validate_name("bad name").is_err());
}

#[test]
fn sqlite_server_round_trip() {
    let temp = TempDir::new().unwrap();
    let context = context(&temp);
    let secret = encrypt(&context, b"p@ssw0rd").unwrap();
    upsert_server(
        &context,
        &Server {
            name: "prod".to_owned(),
            host: "10.0.0.8".to_owned(),
            port: 2222,
            username: "root".to_owned(),
            auth_type: "password".to_owned(),
            key_path: None,
            secret: Some(secret.clone()),
        },
    )
    .unwrap();
    let servers = load_servers(&context).unwrap();
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].name, "prod");
    assert_eq!(servers[0].port, 2222);
    assert_eq!(
        decrypt(&context, servers[0].secret.as_deref().unwrap()).unwrap(),
        b"p@ssw0rd"
    );
    remove_server(&context, "prod").unwrap();
    assert!(load_servers(&context).unwrap().is_empty());
}

#[test]
fn validate_name_rejects_long_names() {
    assert!(validate_name(&"a".repeat(65)).is_err());
}

#[test]
fn remove_server_rejects_missing_name() {
    let temp = TempDir::new().unwrap();
    let context = context(&temp);
    assert!(remove_server(&context, "missing").is_err());
}

#[test]
fn malformed_store_reports_query_errors() {
    let temp = TempDir::new().unwrap();
    let context = context(&temp);
    fs::create_dir_all(&context.data_dir).unwrap();
    let connection = rusqlite::Connection::open(context.data_dir.join("servers.sqlite3")).unwrap();
    connection
        .execute_batch("CREATE TABLE servers (name TEXT PRIMARY KEY)")
        .unwrap();
    drop(connection);

    assert!(load_servers(&context).is_err());
    assert!(
        upsert_server(
            &context,
            &Server {
                name: "prod".to_owned(),
                host: "10.0.0.8".to_owned(),
                port: 22,
                username: "root".to_owned(),
                auth_type: "password".to_owned(),
                key_path: None,
                secret: None,
            },
        )
        .is_err()
    );
}

#[cfg(unix)]
#[test]
fn open_database_reports_readonly_store_error() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempDir::new().unwrap();
    let context = context(&temp);
    fs::create_dir_all(&context.data_dir).unwrap();
    let database = context.data_dir.join("servers.sqlite3");
    {
        let connection = rusqlite::Connection::open(&database).unwrap();
        drop(connection);
    }
    fs::set_permissions(&database, fs::Permissions::from_mode(0o444)).unwrap();
    assert!(open_database(&context).is_err());
    fs::set_permissions(&database, fs::Permissions::from_mode(0o644)).unwrap();
}
