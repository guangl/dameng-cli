//! Import validation: document shape, duplicate names and stored secrets.

use super::*;
use dm_plugin_db::{
    EXPORT_VERSION, PortableConnection, upsert_connection, write_private_file,
    write_private_file_with,
};
use std::fs;
use std::io::Write;

#[test]
fn import_validates_document_shape_and_connection_records() {
    let temp = TempDir::new().unwrap();
    let context = context(&temp, "target");
    let mut export = ExportDocument {
        version: EXPORT_VERSION,
        count: 1,
        connections: Some(vec![PortableConnection {
            name: "bad/name".to_owned(),
            host: "127.0.0.1".to_owned(),
            port: DEFAULT_PORT,
            username: DEFAULT_USERNAME.to_owned(),
            schema: None,
            driver: DEFAULT_DRIVER.to_owned(),
            password: None,
        }]),
        encrypted_payload: None,
        salt: None,
    };
    assert!(import_document(&context, export.clone(), false, None).is_err());
    export.version += 1;
    assert!(import_document(&context, export, false, None).is_err());

    for (username, driver) in [(" ", DEFAULT_DRIVER), (DEFAULT_USERNAME, "\t")] {
        let invalid = ExportDocument {
            version: EXPORT_VERSION,
            count: 1,
            connections: Some(vec![PortableConnection {
                name: "test".to_owned(),
                host: "127.0.0.1".to_owned(),
                port: DEFAULT_PORT,
                username: username.to_owned(),
                schema: None,
                driver: driver.to_owned(),
                password: None,
            }]),
            encrypted_payload: None,
            salt: None,
        };
        assert!(import_document(&context, invalid, false, None).is_err());
    }
}

#[test]
fn import_refuses_collisions_unless_replaced_and_preserves_password() {
    let temp = TempDir::new().unwrap();
    let context = context(&temp, "target");
    upsert_connection(
        &context,
        &connection("prod", Some("local-secret"), &context),
    )
    .unwrap();
    let export = ExportDocument {
        version: EXPORT_VERSION,
        count: 1,
        connections: Some(vec![PortableConnection {
            name: "prod".to_owned(),
            host: "new-host".to_owned(),
            port: DEFAULT_PORT,
            username: DEFAULT_USERNAME.to_owned(),
            schema: None,
            driver: DEFAULT_DRIVER.to_owned(),
            password: None,
        }]),
        encrypted_payload: None,
        salt: None,
    };
    assert!(import_document(&context, export, false, None).is_err());
    let export = ExportDocument {
        version: EXPORT_VERSION,
        count: 1,
        connections: Some(vec![PortableConnection {
            name: "prod".to_owned(),
            host: "new-host".to_owned(),
            port: DEFAULT_PORT,
            username: DEFAULT_USERNAME.to_owned(),
            schema: None,
            driver: DEFAULT_DRIVER.to_owned(),
            password: None,
        }]),
        encrypted_payload: None,
        salt: None,
    };
    import_document(&context, export, true, None).unwrap();
    let saved = find_connection(&context, "prod").unwrap();
    assert_eq!(saved.host, "new-host");
    assert_eq!(
        decrypt(&context, saved.secret.as_deref().unwrap()).unwrap(),
        b"local-secret"
    );
}

#[test]
fn export_file_is_private_and_never_overwritten() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("connections.json");
    write_private_file(&path, b"first").unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"first");
    assert!(write_private_file(&path, b"second").is_err());
    let partial = temp.path().join("partial.json");
    let error = write_private_file_with(&partial, b"contents", |output, contents| {
        output.write_all(&contents[..1])?;
        Err(std::io::Error::other("simulated write failure"))
    });
    assert!(error.is_err());
    assert!(!partial.exists());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}
