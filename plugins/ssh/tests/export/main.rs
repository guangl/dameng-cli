//! Tests for the server export and import documents: plain exports,
//! passphrase-encrypted exports and the rules an import enforces.

use anyhow::Result;
use dm_plugin_sdk::Context as PluginContext;
use dm_plugin_ssh::{
    ExportDocument, Server, decrypt, encrypt, export_document, import_document, load_servers,
};
use std::cell::RefCell;
use tempfile::TempDir;

mod documents;
mod records;
mod shape;

struct SecretPrompter(RefCell<Vec<String>>);

impl dm_plugin_ssh::Prompter for SecretPrompter {
    fn line(&self, _prompt: &str) -> Result<String> {
        anyhow::bail!("Unexpected text prompt")
    }

    fn secret(&self, _prompt: &str) -> Result<String> {
        self.0
            .borrow_mut()
            .pop()
            .ok_or_else(|| anyhow::anyhow!("No scripted secret"))
    }
}

fn context(temp: &TempDir, id: &str) -> PluginContext {
    let home = temp.path().join(id);
    PluginContext {
        args: vec![],
        plugin_dir: home.join("plugins/ssh"),
        config_dir: home.join("config/ssh"),
        data_dir: home.join("data/ssh"),
        cache_dir: home.join("cache/ssh"),
        home,
        capabilities: vec![],
    }
}

fn server(
    name: &str,
    auth_type: &str,
    key_path: Option<&str>,
    secret: Option<&str>,
    context: &PluginContext,
) -> Server {
    Server {
        name: name.to_owned(),
        host: "10.0.0.8".to_owned(),
        port: 2222,
        username: "root".to_owned(),
        auth_type: auth_type.to_owned(),
        key_path: key_path.map(str::to_owned),
        secret: secret.map(|value| encrypt(context, value.as_bytes()).unwrap()),
    }
}

fn prompter(secret: &str) -> SecretPrompter {
    SecretPrompter(RefCell::new(vec![secret.to_owned()]))
}

/// Answers "Export passphrase: " and "Confirm export passphrase: " in order.
fn confirmed(first: &str, confirmation: &str) -> SecretPrompter {
    SecretPrompter(RefCell::new(vec![
        confirmation.to_owned(),
        first.to_owned(),
    ]))
}

#[test]
fn plain_export_omits_secrets_and_round_trips_configuration() {
    let temp = TempDir::new().unwrap();
    let source = context(&temp, "source");
    let export = export_document(
        &source,
        vec![
            server("prod", "password", None, Some("p@ssw0rd"), &source),
            server(
                "keyed",
                "key",
                Some("/tmp/id_ed25519"),
                Some("key-passphrase"),
                &source,
            ),
        ],
        false,
        None,
    )
    .unwrap();
    let json = serde_json::to_string(&export).unwrap();
    assert!(!json.contains("p@ssw0rd"), "{json}");
    assert!(!json.contains("key-passphrase"), "{json}");
    let imported: ExportDocument = serde_json::from_str(&json).unwrap();
    let target = context(&temp, "target");
    assert_eq!(import_document(&target, imported, false, None).unwrap(), 2);
    let saved = load_servers(&target).unwrap();
    let prod = saved.iter().find(|item| item.name == "prod").unwrap();
    assert_eq!(prod.host, "10.0.0.8");
    assert_eq!(prod.port, 2222);
    assert_eq!(prod.username, "root");
    assert_eq!(prod.auth_type, "password");
    assert!(prod.secret.is_none());
    let keyed = saved.iter().find(|item| item.name == "keyed").unwrap();
    assert_eq!(keyed.auth_type, "key");
    assert_eq!(keyed.key_path.as_deref(), Some("/tmp/id_ed25519"));
    assert!(keyed.secret.is_none());
}

#[test]
fn encrypted_export_import_reencrypts_secret_for_destination() {
    let temp = TempDir::new().unwrap();
    let source = context(&temp, "source");
    let target = context(&temp, "target");
    let source_server = server("prod", "password", None, Some("p@ssw0rd"), &source);
    let source_secret = source_server.secret.clone().unwrap();
    let encrypted = export_document(
        &source,
        vec![source_server],
        true,
        Some(&confirmed("transfer-passphrase", "transfer-passphrase")),
    )
    .unwrap();
    let json = serde_json::to_string(&encrypted).unwrap();
    assert!(!json.contains("p@ssw0rd"), "{json}");
    let document: ExportDocument = serde_json::from_str(&json).unwrap();
    import_document(
        &target,
        document,
        false,
        Some(&prompter("transfer-passphrase")),
    )
    .unwrap();
    let saved = load_servers(&target).unwrap();
    assert_eq!(
        decrypt(&target, saved[0].secret.as_deref().unwrap()).unwrap(),
        b"p@ssw0rd"
    );
    assert_ne!(saved[0].secret.as_deref(), Some(source_secret.as_str()));
}

#[test]
fn encrypted_import_rejects_wrong_passphrase_and_missing_terminal() {
    let temp = TempDir::new().unwrap();
    let source = context(&temp, "source");
    let export = export_document(
        &source,
        vec![server("prod", "password", None, Some("p@ssw0rd"), &source)],
        true,
        Some(&confirmed("right", "right")),
    )
    .unwrap();
    assert!(
        import_document(
            &context(&temp, "wrong"),
            export,
            false,
            Some(&prompter("wrong"))
        )
        .is_err()
    );

    assert!(export_document(&source, vec![], true, None).is_err());
    assert!(export_document(&source, vec![], true, Some(&confirmed("first", "other"))).is_err());
    assert!(export_document(&source, vec![], true, Some(&prompter(""))).is_err());
}

#[test]
fn a_password_server_never_exports_a_stale_key_path() {
    let temp = TempDir::new().unwrap();
    let source = context(&temp, "source");
    let export = export_document(
        &source,
        vec![server(
            "prod",
            "password",
            Some("/tmp/stale"),
            Some("p@ssw0rd"),
            &source,
        )],
        false,
        None,
    )
    .unwrap();
    // A stale key path on a password server stays out of the document, so the
    // importer never sees an inconsistent record.
    assert!(export.servers.unwrap()[0].key_path.is_none());
    assert!(export.encrypted_payload.is_none());
}
