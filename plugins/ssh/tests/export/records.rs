//! Import collisions, retained secrets and the export file itself.

use super::documents::*;
use super::*;
use dm_plugin_ssh::{upsert_server, write_private_file, write_private_file_with};
use std::fs;
use std::io::Write;

#[test]
fn import_refuses_collisions_unless_replaced_and_keeps_only_matching_secrets() {
    let temp = TempDir::new().unwrap();
    let context = context(&temp, "target");
    for (name, auth, key, secret) in [
        ("prod", "password", None, Some("local-secret")),
        ("keyed", "key", Some("/tmp/same"), Some("passphrase-secret")),
        ("other-key", "key", Some("/tmp/old"), Some("old-passphrase")),
    ] {
        upsert_server(&context, &server(name, auth, key, secret, &context)).unwrap();
    }

    let imported = document(vec![
        PortableServer {
            host: "new-host".to_owned(),
            ..portable("prod")
        },
        PortableServer {
            auth_type: "key".to_owned(),
            key_path: Some("/tmp/same".to_owned()),
            ..portable("keyed")
        },
        PortableServer {
            auth_type: "key".to_owned(),
            key_path: Some("/tmp/new".to_owned()),
            ..portable("other-key")
        },
    ]);
    assert!(import_document(&context, imported.clone(), false, None).is_err());
    assert_eq!(import_document(&context, imported, true, None).unwrap(), 3);

    let saved = load_servers(&context).unwrap();
    let by_name = |name: &str| saved.iter().find(|item| item.name == name).unwrap();
    let prod = by_name("prod");
    assert_eq!(prod.host, "new-host");
    assert_eq!(
        decrypt(&context, prod.secret.as_deref().unwrap()).unwrap(),
        b"local-secret"
    );
    let keyed = by_name("keyed");
    assert_eq!(keyed.key_path.as_deref(), Some("/tmp/same"));
    assert_eq!(
        decrypt(&context, keyed.secret.as_deref().unwrap()).unwrap(),
        b"passphrase-secret"
    );
    let other = by_name("other-key");
    assert_eq!(other.key_path.as_deref(), Some("/tmp/new"));
    assert!(other.secret.is_none(), "{:?}", other.secret);
}

#[test]
fn import_clears_a_secret_when_the_authentication_method_changes() {
    let temp = TempDir::new().unwrap();
    let context = context(&temp, "target");
    upsert_server(
        &context,
        &server("prod", "password", None, Some("local-secret"), &context),
    )
    .unwrap();
    let imported = document(vec![PortableServer {
        auth_type: "key".to_owned(),
        key_path: Some("/tmp/id_ed25519".to_owned()),
        ..portable("prod")
    }]);
    import_document(&context, imported, true, None).unwrap();
    let saved = load_servers(&context).unwrap();
    assert_eq!(saved[0].auth_type, "key");
    assert!(saved[0].secret.is_none());
}

#[test]
fn export_file_is_private_and_never_overwritten() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("servers.json");
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
