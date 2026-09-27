//! Export documents built by hand, so import rules can be tested directly.

pub use dm_plugin_ssh::{EXPORT_VERSION, ExportDocument, PortableServer};

pub fn portable(name: &str) -> PortableServer {
    PortableServer {
        name: name.to_owned(),
        host: "127.0.0.1".to_owned(),
        port: 22,
        username: "root".to_owned(),
        auth_type: "password".to_owned(),
        key_path: None,
        secret: None,
    }
}

pub fn document(servers: Vec<PortableServer>) -> ExportDocument {
    ExportDocument {
        version: EXPORT_VERSION,
        count: servers.len(),
        servers: Some(servers),
        encrypted_payload: None,
        salt: None,
    }
}

pub fn encrypted(payload: &str, salt: &str) -> ExportDocument {
    ExportDocument {
        version: EXPORT_VERSION,
        count: 0,
        servers: None,
        encrypted_payload: Some(payload.to_owned()),
        salt: Some(salt.to_owned()),
    }
}
