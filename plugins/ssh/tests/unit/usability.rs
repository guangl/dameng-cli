use crate::common::*;
use dm_plugin_ssh::{decrypt, load_servers, run_with_prompter};
use tempfile::TempDir;
fn run(context: &mut dm_plugin_sdk::Context, args: &[&str]) -> anyhow::Result<i32> {
    context.args = args.iter().map(std::ffi::OsString::from).collect();
    run_with_prompter(context, None)
}
#[test]
fn edits_preserve_secrets_and_auth_changes_do_not_reuse_them() {
    let temp = TempDir::new().unwrap();
    let mut context = context(&temp);
    run(
        &mut context,
        &[
            "add",
            "prod",
            "--host",
            "h",
            "--username",
            "root",
            "--password",
            "pw",
        ],
    )
    .unwrap();
    assert!(
        run(
            &mut context,
            &["add", "prod", "--host", "new", "--password", "pw"]
        )
        .is_err()
    );
    run(&mut context, &["edit", "prod", "--port", "2222"]).unwrap();
    let entries = load_servers(&context).unwrap();
    assert_eq!(entries[0].port, 2222);
    assert_eq!(
        decrypt(&context, entries[0].secret.as_deref().unwrap()).unwrap(),
        b"pw"
    );
    run(
        &mut context,
        &["edit", "prod", "--key", "id_rsa", "--passphrase", "phrase"],
    )
    .unwrap();
    run(&mut context, &["edit", "prod", "--key", "id_rsa"]).unwrap();
    let entries = load_servers(&context).unwrap();
    assert_eq!(
        decrypt(&context, entries[0].secret.as_deref().unwrap()).unwrap(),
        b"phrase"
    );
    run(&mut context, &["edit", "prod", "--passphrase", "new"]).unwrap();
    run(&mut context, &["edit", "prod", "--passphrase", ""]).unwrap();
    assert!(load_servers(&context).unwrap()[0].secret.is_none());
    run(&mut context, &["edit", "prod", "--password", "newpw"]).unwrap();
    assert_eq!(load_servers(&context).unwrap()[0].auth_type, "password");
    assert!(run(&mut context, &["edit", "prod", "--passphrase", "pw"]).is_err());
    assert!(run(&mut context, &["edit", "missing"]).is_err());
    run(
        &mut context,
        &[
            "add",
            "prod",
            "--replace",
            "--host",
            "new",
            "--username",
            "user",
            "--key",
            "other",
        ],
    )
    .unwrap();
    assert_eq!(
        load_servers(&context).unwrap()[0].key_path.as_deref(),
        Some("other")
    );
}
#[test]
fn interactive_edit_confirms_and_preserves_old_values() {
    let temp = TempDir::new().unwrap();
    let mut context = context(&temp);
    run(
        &mut context,
        &[
            "add",
            "prod",
            "--host",
            "h",
            "--username",
            "root",
            "--password",
            "pw",
        ],
    )
    .unwrap();
    context.args = ["edit", "prod"].map(std::ffi::OsString::from).to_vec();
    let script = Script::new(&["", "", "", "n"], &[]);
    assert!(run_with_prompter(&context, Some(&script)).is_err());
    let script = Script::new(&["new", "user", "2222", "y"], &[]);
    run_with_prompter(&context, Some(&script)).unwrap();
    assert_eq!(load_servers(&context).unwrap()[0].host, "new");
    context.args = [
        "add",
        "other",
        "--host",
        "h",
        "--username",
        "root",
        "--password",
        "pw",
    ]
    .map(std::ffi::OsString::from)
    .to_vec();
    let script = Script::new(&["", "n"], &[]);
    assert!(run_with_prompter(&context, Some(&script)).is_err());
    assert_eq!(load_servers(&context).unwrap().len(), 1);
    assert!(run(&mut context, &["remove", "prod"]).is_err());
}
#[test]
fn config_doctor_and_completion_check_environment_and_saved_keys() {
    let temp = TempDir::new().unwrap();
    let mut context = context(&temp);
    for args in [
        &["config", "path"][..],
        &["config", "init"],
        &["config", "show"],
        &["config", "show", "--json"],
        &["__complete", ""],
    ] {
        run(&mut context, args).unwrap();
    }
    run(
        &mut context,
        &[
            "add",
            "keyed",
            "--host",
            "h",
            "--username",
            "user",
            "--key",
            "missing",
        ],
    )
    .unwrap();
    assert_eq!(run(&mut context, &["doctor", "--json"]).unwrap(), 1);
    let key = temp.path().join("key");
    std::fs::write(&key, "key").unwrap();
    run(
        &mut context,
        &["edit", "keyed", "--key", key.to_str().unwrap()],
    )
    .unwrap();
    run(&mut context, &["doctor"]).unwrap();
    run(&mut context, &["__complete", "connect", "ke"]).unwrap();
    std::fs::write(context.config_dir.join("config.toml"), "invalid").unwrap();
    assert_eq!(run(&mut context, &["doctor"]).unwrap(), 1);
    std::fs::remove_file(context.config_dir.join("config.toml")).unwrap();
    std::fs::remove_file(context.data_dir.join("servers.sqlite3")).unwrap();
    std::fs::create_dir(context.data_dir.join("servers.sqlite3")).unwrap();
    assert_eq!(run(&mut context, &["doctor"]).unwrap(), 1);
}
