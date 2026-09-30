use dm_plugin_ssh::{Server, encrypt, ssh_command, upsert_server};
use tempfile::TempDir;

use crate::common::*;

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
    assert_eq!(&args[..2], ["-e", "ssh"]);
    assert!(!args.iter().any(|arg| arg.contains("p@ssw0rd")));
    assert!(command.get_envs().any(|(key, value)| {
        key == "SSHPASS" && value.is_some_and(|value| value == "p@ssw0rd")
    }));
    assert!(args.contains(&"root@10.0.0.8".to_owned()));
    assert!(args.contains(&"true".to_owned()));
}

#[test]
fn saved_key_passphrase_is_used_for_tests_and_login_without_entering_arguments() {
    let temp = TempDir::new().unwrap();
    let context = context(&temp);
    upsert_server(
        &context,
        &Server {
            name: "encrypted".into(),
            host: "example.invalid".into(),
            port: 2222,
            username: "user".into(),
            auth_type: "key".into(),
            key_path: Some("/tmp/private key".into()),
            secret: Some(encrypt(&context, b"hidden-passphrase").unwrap()),
        },
    )
    .unwrap();
    for test in [true, false] {
        let command = ssh_command(&context, "encrypted", test).unwrap();
        assert_eq!(command.get_program(), "sshpass");
        let args: Vec<_> = command
            .get_args()
            .map(|arg| arg.to_str().unwrap())
            .collect();
        assert_eq!(&args[..4], ["-e", "-P", "Enter passphrase for key", "ssh"]);
        assert!(args.contains(&"/tmp/private key"));
        assert!(!args.iter().any(|arg| arg.contains("hidden-passphrase")));
        if test {
            assert!(args.contains(&"BatchMode=no"));
            assert!(args.contains(&"ConnectTimeout=10"));
        }
        for (key, expected) in [("SSHPASS", "hidden-passphrase"), ("LC_ALL", "C")] {
            assert!(command.get_envs().any(|(name, value)| {
                name == key && value.is_some_and(|value| value == expected)
            }));
        }
    }
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
