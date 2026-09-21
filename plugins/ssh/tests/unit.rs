use std::ffi::OsString;
use std::fs;

use dm_plugin_sdk::Context as PluginContext;
use dm_plugin_ssh::{
    Server, decrypt, encrypt, hex, load_servers, machine_key, open_database, remove_server,
    resolve_auth, resolve_passphrase, resolve_password, resolve_port, resolve_required,
    ssh_command, unhex, upsert_server, validate_name,
};
use tempfile::TempDir;

fn context(temp: &TempDir) -> PluginContext {
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    PluginContext {
        args: vec![OsString::from("ssh")],
        plugin_dir: temp.path().join("plugin"),
        home,
        config_dir: temp.path().join("config/ssh"),
        data_dir: temp.path().join("data/ssh"),
        cache_dir: temp.path().join("cache/ssh"),
        capabilities: vec!["config-dirs-v1".to_owned()],
    }
}

#[test]
fn hex_round_trip() {
    let bytes = b"secret-value";
    assert_eq!(unhex(&hex(bytes)).unwrap(), bytes);
}

#[test]
fn validate_name_accepts_and_rejects() {
    assert!(validate_name("prod-01").is_ok());
    assert!(validate_name("").is_err());
    assert!(validate_name("../bad").is_err());
    assert!(validate_name("bad name").is_err());
}

#[test]
fn encrypt_decrypt_round_trip() {
    let temp = TempDir::new().unwrap();
    let context = context(&temp);
    let encrypted = encrypt(&context, b"p@ssw0rd").unwrap();
    assert_ne!(encrypted, "p@ssw0rd");
    assert_eq!(decrypt(&context, &encrypted).unwrap(), b"p@ssw0rd");
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
fn ssh_command_password_uses_sshpass() {
    let temp = TempDir::new().unwrap();
    let context = context(&temp);
    let secret = encrypt(&context, b"p@ssw0rd").unwrap();
    upsert_server(
        &context,
        &Server {
            name: "prod".to_owned(),
            host: "10.0.0.8".to_owned(),
            port: 22,
            username: "root".to_owned(),
            auth_type: "password".to_owned(),
            key_path: None,
            secret: Some(secret),
        },
    )
    .unwrap();
    let command = ssh_command(&context, "prod", true).unwrap();
    assert_eq!(command.get_program(), "sshpass");
    let args: Vec<String> = command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    assert!(args.contains(&"-p".to_owned()));
    assert!(args.contains(&"p@ssw0rd".to_owned()));
    assert!(args.contains(&"root@10.0.0.8".to_owned()));
    assert!(args.contains(&"true".to_owned()));
}

#[test]
fn ssh_command_key_uses_ssh() {
    let temp = TempDir::new().unwrap();
    let context = context(&temp);
    upsert_server(
        &context,
        &Server {
            name: "prod".to_owned(),
            host: "10.0.0.8".to_owned(),
            port: 22,
            username: "root".to_owned(),
            auth_type: "key".to_owned(),
            key_path: Some("/tmp/id_ed25519".to_owned()),
            secret: None,
        },
    )
    .unwrap();
    let command = ssh_command(&context, "prod", false).unwrap();
    assert_eq!(command.get_program(), "ssh");
    let args: Vec<String> = command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    assert!(args.contains(&"-i".to_owned()));
    assert!(args.contains(&"/tmp/id_ed25519".to_owned()));
    assert!(args.contains(&"root@10.0.0.8".to_owned()));
}

#[test]
fn machine_key_rejects_invalid_key_length() {
    let temp = TempDir::new().unwrap();
    let context = context(&temp);
    fs::create_dir_all(&context.data_dir).unwrap();
    fs::write(context.data_dir.join(".ssh-key"), b"too short").unwrap();
    assert!(machine_key(&context).is_err());
}

#[test]
fn machine_key_rejects_unreadable_key() {
    let temp = TempDir::new().unwrap();
    let context = context(&temp);
    fs::create_dir_all(&context.data_dir).unwrap();
    fs::create_dir(context.data_dir.join(".ssh-key")).unwrap();
    assert!(machine_key(&context).is_err());
}

#[test]
fn decrypt_rejects_short_and_corrupt_text() {
    let temp = TempDir::new().unwrap();
    let context = context(&temp);
    assert!(decrypt(&context, "aa").is_err());
    let secret = encrypt(&context, b"p@ssw0rd").unwrap();
    let mut bytes = unhex(&secret).unwrap();
    bytes.truncate(12);
    bytes.extend_from_slice(&[0_u8; 16]);
    assert!(decrypt(&context, &hex(&bytes)).is_err());
}

#[test]
fn unhex_rejects_invalid_input() {
    assert!(unhex("0").is_err());
    assert!(unhex("zz").is_err());
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
fn ssh_command_rejects_missing_key_and_password() {
    let temp = TempDir::new().unwrap();
    let context = context(&temp);
    upsert_server(
        &context,
        &Server {
            name: "nokey".to_owned(),
            host: "10.0.0.8".to_owned(),
            port: 22,
            username: "root".to_owned(),
            auth_type: "key".to_owned(),
            key_path: None,
            secret: None,
        },
    )
    .unwrap();
    assert!(ssh_command(&context, "nokey", false).is_err());

    upsert_server(
        &context,
        &Server {
            name: "nopass".to_owned(),
            host: "10.0.0.8".to_owned(),
            port: 22,
            username: "root".to_owned(),
            auth_type: "password".to_owned(),
            key_path: None,
            secret: None,
        },
    )
    .unwrap();
    assert!(ssh_command(&context, "nopass", false).is_err());
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

#[test]
fn resolve_password_uses_explicit_value_without_prompting() {
    assert_eq!(
        resolve_password(Some("p@ssw0rd".to_owned()), true).unwrap(),
        "p@ssw0rd"
    );
    assert_eq!(
        resolve_password(Some("p@ssw0rd".to_owned()), false).unwrap(),
        "p@ssw0rd"
    );
}

#[test]
fn resolve_password_rejects_empty_explicit_value() {
    assert!(resolve_password(Some(String::new()), false).is_err());
}

#[test]
fn resolve_password_requires_value_when_not_interactive() {
    let error = resolve_password(None, false).unwrap_err();
    assert!(
        error.to_string().contains("password or key path"),
        "{error}"
    );
}

#[test]
fn resolve_passphrase_handles_explicit_and_missing_values() {
    assert_eq!(
        resolve_passphrase(Some("secret".to_owned()), false)
            .unwrap()
            .as_deref(),
        Some("secret")
    );
    assert_eq!(resolve_passphrase(None, false).unwrap(), None);
    assert_eq!(
        resolve_passphrase(Some(String::new()), false).unwrap(),
        None
    );
}

#[test]
fn resolve_required_uses_value_or_reports_missing() {
    assert_eq!(
        resolve_required(Some("prod".to_owned()), "Name: ", "missing", false).unwrap(),
        "prod"
    );
    let error = resolve_required(None, "Name: ", "name is required", false).unwrap_err();
    assert!(error.to_string().contains("name is required"), "{error}");
}

#[test]
fn resolve_port_defaults_to_22_and_accepts_explicit_value() {
    assert_eq!(resolve_port(None, false).unwrap(), 22);
    assert_eq!(resolve_port(Some(2222), false).unwrap(), 2222);
}

#[test]
fn resolve_auth_uses_explicit_password() {
    let temp = TempDir::new().unwrap();
    let context = context(&temp);
    let (auth_type, key_path, secret) =
        resolve_auth(&context, Some("p@ssw0rd".to_owned()), None, None, false).unwrap();
    assert_eq!(auth_type, "password");
    assert_eq!(key_path, None);
    assert_eq!(
        decrypt(&context, secret.as_deref().unwrap()).unwrap(),
        b"p@ssw0rd"
    );
}

#[test]
fn resolve_auth_uses_explicit_key() {
    let temp = TempDir::new().unwrap();
    let context = context(&temp);
    let (auth_type, key_path, secret) = resolve_auth(
        &context,
        None,
        Some(std::path::PathBuf::from("/tmp/id_ed25519")),
        Some("secret".to_owned()),
        false,
    )
    .unwrap();
    assert_eq!(auth_type, "key");
    assert_eq!(key_path.as_deref(), Some("/tmp/id_ed25519"));
    assert_eq!(
        decrypt(&context, secret.as_deref().unwrap()).unwrap(),
        b"secret"
    );
}

#[test]
fn resolve_auth_requires_secret_when_not_interactive() {
    let temp = TempDir::new().unwrap();
    let context = context(&temp);
    let error = resolve_auth(&context, None, None, None, false).unwrap_err();
    assert!(
        error.to_string().contains("password or key path"),
        "{error}"
    );
}
