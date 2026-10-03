//! Tests for the connection export and import documents: plain exports,
//! passphrase-encrypted exports and the rules an import enforces.

use anyhow::Result;
use dm_plugin_db::{
    DEFAULT_DRIVER, DEFAULT_PORT, DEFAULT_USERNAME, DatabaseConnection, ExportDocument, Prompter,
    decrypt, encrypt, export_document, find_connection, import_document,
};
use dm_plugin_sdk::Context as PluginContext;
use std::cell::RefCell;
use tempfile::TempDir;

mod legacy;
mod records;

struct PasswordPrompter(RefCell<Vec<String>>);

impl Prompter for PasswordPrompter {
    fn line(&self, _prompt: &str) -> Result<String> {
        anyhow::bail!("Unexpected text prompt")
    }

    fn secret(&self, _prompt: &str) -> Result<String> {
        self.0
            .borrow_mut()
            .pop()
            .ok_or_else(|| anyhow::anyhow!("No scripted password"))
    }
}

fn context(temp: &TempDir, id: &str) -> PluginContext {
    let home = temp.path().join(id);
    PluginContext {
        args: vec![],
        plugin_dir: home.join("plugins/db"),
        config_dir: home.join("config/db"),
        data_dir: home.join("data/db"),
        cache_dir: home.join("cache/db"),
        home,
        capabilities: vec![],
    }
}

fn connection(name: &str, password: Option<&str>, context: &PluginContext) -> DatabaseConnection {
    DatabaseConnection {
        name: name.to_owned(),
        host: "127.0.0.1".to_owned(),
        port: DEFAULT_PORT,
        username: DEFAULT_USERNAME.to_owned(),
        schema: Some("DMHR".to_owned()),
        driver: DEFAULT_DRIVER.to_owned(),
        secret: password.map(|value| encrypt(context, value.as_bytes()).unwrap()),
    }
}

fn prompter(password: &str) -> PasswordPrompter {
    PasswordPrompter(RefCell::new(vec![password.to_owned()]))
}

fn confirmed_prompter(password: &str, confirmation: &str) -> PasswordPrompter {
    PasswordPrompter(RefCell::new(vec![
        confirmation.to_owned(),
        password.to_owned(),
    ]))
}

#[test]
fn plain_export_omits_passwords_and_round_trips_configuration() {
    let temp = TempDir::new().unwrap();
    let source = context(&temp, "source");
    let export = export_document(
        &source,
        vec![connection("prod", Some("secret"), &source)],
        false,
        None,
    )
    .unwrap();
    let json = serde_json::to_string(&export).unwrap();
    assert!(!json.contains("secret"));
    let imported: ExportDocument = serde_json::from_str(&json).unwrap();
    let target = context(&temp, "target");
    assert_eq!(import_document(&target, imported, false, None).unwrap(), 1);
    let saved = find_connection(&target, "prod").unwrap();
    assert_eq!(saved.host, "127.0.0.1");
    assert!(saved.secret.is_none());
}

#[test]
fn encrypted_export_import_reencrypts_password_for_destination() {
    let temp = TempDir::new().unwrap();
    let source = context(&temp, "source");
    let target = context(&temp, "target");
    let source_connection = connection("prod", Some("secret"), &source);
    let source_secret = source_connection.secret.clone().unwrap();
    let encrypted = export_document(
        &source,
        vec![source_connection],
        true,
        Some(&confirmed_prompter(
            "transfer-passphrase",
            "transfer-passphrase",
        )),
    )
    .unwrap();
    let json = serde_json::to_string(&encrypted).unwrap();
    assert!(!json.contains("secret"));
    let document: ExportDocument = serde_json::from_str(&json).unwrap();
    import_document(
        &target,
        document,
        false,
        Some(&prompter("transfer-passphrase")),
    )
    .unwrap();
    let saved = find_connection(&target, "prod").unwrap();
    assert_eq!(
        decrypt(&target, saved.secret.as_deref().unwrap()).unwrap(),
        b"secret"
    );
    assert_ne!(saved.secret.as_deref(), Some(source_secret.as_str()));
}

#[test]
fn encrypted_import_rejects_wrong_passphrase_and_missing_terminal() {
    let temp = TempDir::new().unwrap();
    let source = context(&temp, "source");
    let export = export_document(
        &source,
        vec![connection("prod", Some("secret"), &source)],
        true,
        Some(&confirmed_prompter("right", "right")),
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

    let no_passwords = export_document(&source, vec![], true, None);
    assert!(no_passwords.is_err());
    assert!(
        export_document(
            &source,
            vec![],
            true,
            Some(&confirmed_prompter("first", "different")),
        )
        .is_err()
    );
}
