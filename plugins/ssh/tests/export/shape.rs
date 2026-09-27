//! The document shapes an import accepts, and the ones it refuses.

use super::documents::*;
use super::*;

#[test]
fn import_validates_document_shape_and_server_records() {
    let temp = TempDir::new().unwrap();
    let context = context(&temp, "target");

    assert!(import_document(&context, document(vec![portable("bad/name")]), false, None).is_err());

    let mut outdated = document(vec![portable("prod")]);
    outdated.version += 1;
    assert!(import_document(&context, outdated, false, None).is_err());

    let mut miscounted = document(vec![portable("prod")]);
    miscounted.count += 1;
    assert!(import_document(&context, miscounted, false, None).is_err());

    for broken in [
        PortableServer {
            host: "  ".to_owned(),
            ..portable("prod")
        },
        PortableServer {
            username: String::new(),
            ..portable("prod")
        },
        PortableServer {
            port: 0,
            ..portable("prod")
        },
        PortableServer {
            auth_type: "token".to_owned(),
            ..portable("prod")
        },
        PortableServer {
            auth_type: "key".to_owned(),
            ..portable("prod")
        },
        PortableServer {
            key_path: Some("/tmp/id_ed25519".to_owned()),
            ..portable("prod")
        },
    ] {
        assert!(import_document(&context, document(vec![broken]), false, None).is_err());
    }

    let duplicates = document(vec![portable("prod"), portable("prod")]);
    assert!(import_document(&context, duplicates, false, None).is_err());

    // A plain document must never smuggle a secret through.
    let smuggled = PortableServer {
        secret: Some("p@ssw0rd".to_owned()),
        ..portable("prod")
    };
    assert!(import_document(&context, document(vec![smuggled]), false, None).is_err());

    // Neither plain servers nor an encrypted payload.
    let neither = ExportDocument {
        version: EXPORT_VERSION,
        count: 0,
        servers: None,
        encrypted_payload: None,
        salt: None,
    };
    assert!(import_document(&context, neither, false, None).is_err());

    let mut both = document(vec![portable("prod")]);
    both.encrypted_payload = Some("00".repeat(13));
    both.salt = Some("00".repeat(16));
    assert!(import_document(&context, both, false, None).is_err());

    for broken in [
        encrypted(&"00".repeat(13), "00"),
        encrypted("00", &"00".repeat(16)),
        encrypted("zz00", &"00".repeat(16)),
        encrypted(&"00".repeat(13), &"00".repeat(15)),
    ] {
        assert!(import_document(&context, broken, false, Some(&prompter("pass"))).is_err());
    }
}
