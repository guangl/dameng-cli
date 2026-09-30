use dm_plugin_sdk::Context as PluginContext;
use dm_plugin_ssh::{decrypt, load_servers, run_with_prompter};
use std::ffi::OsString;
use tempfile::TempDir;

use crate::common::*;

/// A plugin context whose store is named by "id", with these command arguments.
fn store(temp: &TempDir, id: &str, args: &[&str]) -> PluginContext {
    let root = temp.path().join(id);
    PluginContext {
        args: args.iter().map(OsString::from).collect(),
        plugin_dir: root.join("plugin"),
        home: root.join("home"),
        config_dir: root.join("config/ssh"),
        data_dir: root.join("data/ssh"),
        cache_dir: root.join("cache/ssh"),
        capabilities: vec!["config-dirs-v1".to_owned()],
    }
}

#[test]
fn add_prompts_for_every_omitted_value() {
    let temp = TempDir::new().unwrap();
    let script = Script::new(
        &["prod", "10.0.0.8", "2222", "root", "password"],
        &["p@ssw0rd"],
    );
    let context = store(&temp, "source", &["add", "--yes"]);
    assert_eq!(run_with_prompter(&context, Some(&script)).unwrap(), 0);
    let saved = load_servers(&context).unwrap();
    assert_eq!(saved[0].name, "prod");
    assert_eq!(saved[0].port, 2222);
    assert_eq!(
        decrypt(&context, saved[0].secret.as_deref().unwrap()).unwrap(),
        b"p@ssw0rd"
    );

    // Key authentication with an empty passphrase stores no secret.
    let script = Script::new(
        &["keyed", "10.0.0.9", "", "ubuntu", "key", "/tmp/id_ed25519"],
        &[""],
    );
    assert_eq!(run_with_prompter(&context, Some(&script)).unwrap(), 0);
    let saved = load_servers(&context).unwrap();
    let keyed = saved.iter().find(|item| item.name == "keyed").unwrap();
    assert_eq!(keyed.auth_type, "key");
    assert_eq!(keyed.key_path.as_deref(), Some("/tmp/id_ed25519"));
    assert!(keyed.secret.is_none());
}

#[test]
fn export_import_and_remove_dispatch_through_the_cli() {
    let temp = TempDir::new().unwrap();
    let source = store(
        &temp,
        "source",
        &[
            "add",
            "prod",
            "--host",
            "10.0.0.8",
            "--port",
            "2222",
            "--username",
            "root",
        ],
    );
    let script = Script::new(&["password", "y"], &["p@ssw0rd"]);
    assert_eq!(run_with_prompter(&source, Some(&script)).unwrap(), 0);

    let file = temp.path().join("servers.json");
    let path = file.to_str().unwrap().to_owned();
    let script = Script::new(&[], &["transfer", "transfer"]);
    let export = store(
        &temp,
        "source",
        &["export", "--file", &path, "--include-secrets"],
    );
    assert_eq!(run_with_prompter(&export, Some(&script)).unwrap(), 0);

    let script = Script::new(&[], &["transfer"]);
    let target = store(&temp, "target", &["import", &path]);
    assert_eq!(run_with_prompter(&target, Some(&script)).unwrap(), 0);
    let saved = load_servers(&target).unwrap();
    assert_eq!(saved.len(), 1);
    assert_eq!(
        decrypt(&target, saved[0].secret.as_deref().unwrap()).unwrap(),
        b"p@ssw0rd"
    );

    let remove = store(&temp, "target", &["remove", "prod", "--yes"]);
    assert_eq!(run_with_prompter(&remove, None).unwrap(), 0);
    assert!(load_servers(&remove).unwrap().is_empty());
}

#[test]
fn commands_report_failures_without_a_terminal() {
    let temp = TempDir::new().unwrap();
    let add = store(&temp, "store", &["add", "--yes"]);
    assert!(run_with_prompter(&add, None).is_err());
    let remove = store(&temp, "store", &["remove", "missing"]);
    assert!(run_with_prompter(&remove, None).is_err());
    let export = store(&temp, "store", &["export", "--include-secrets"]);
    assert!(run_with_prompter(&export, None).is_err());
    let import = store(&temp, "store", &["import", "/nonexistent/servers.json"]);
    assert!(run_with_prompter(&import, None).is_err());
}
