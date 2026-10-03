use crate::common::*;
use dm_plugin_ssh::{
    Server, encrypt, load_servers, run_with_validator, server_by_name, test_server, upsert_server,
};
use tempfile::TempDir;

#[test]
fn failed_add_and_replace_never_persist_candidate() {
    let temp = TempDir::new().unwrap();
    let mut context = context(&temp);
    context.args = [
        "add",
        "prod",
        "--host",
        "localhost",
        "--username",
        "user",
        "--password",
        "secret",
    ]
    .map(std::ffi::OsString::from)
    .to_vec();
    let failure =
        |_: &dm_plugin_sdk::Context, _: &Server| anyhow::bail!("SSH authentication failed");
    let error = run_with_validator(&context, None, &failure).unwrap_err();
    assert!(error.to_string().contains("配置未保存"));
    assert!(load_servers(&context).unwrap().is_empty());
    let old = Server {
        name: "prod".into(),
        host: "old".into(),
        port: 22,
        username: "old".into(),
        auth_type: "password".into(),
        key_path: None,
        secret: Some(encrypt(&context, b"old-password").unwrap()),
    };
    upsert_server(&context, &old).unwrap();
    context.args.push("--replace".into());
    assert!(run_with_validator(&context, None, &failure).is_err());
    let saved = server_by_name(&context, "prod").unwrap();
    assert_eq!(saved.host, old.host);
    assert_eq!(saved.secret, old.secret);
}

#[test]
fn native_connection_refusal_is_reported_without_saving() {
    let temp = TempDir::new().unwrap();
    let context = context(&temp);
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let server = Server {
        name: "refused".into(),
        host: "127.0.0.1".into(),
        port,
        username: "user".into(),
        auth_type: "password".into(),
        key_path: None,
        secret: Some(encrypt(&context, b"hidden").unwrap()),
    };
    let error = test_server(&context, &server).unwrap_err();
    assert!(format!("{error:#}").contains("SSH connection failed"));
    assert!(!format!("{error:#}").contains("hidden"));
    assert!(load_servers(&context).unwrap().is_empty());
    assert!(!context.data_dir.join("known_hosts").exists());
}
