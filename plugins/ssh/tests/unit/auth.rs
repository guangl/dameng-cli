use dm_plugin_ssh::{decrypt, resolve_auth};
use tempfile::TempDir;

use crate::common::*;

#[test]
fn resolve_auth_uses_explicit_password() {
    let temp = TempDir::new().unwrap();
    let context = context(&temp);
    let (auth_type, key_path, secret) = resolve_auth(
        &context,
        Some("p@ssw0rd".to_owned()),
        None,
        None,
        None,
        None,
    )
    .unwrap();
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
        None,
        None,
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
    let error = resolve_auth(&context, None, None, None, None, None).unwrap_err();
    assert!(
        error.to_string().contains("password or key path"),
        "{error}"
    );
}

#[test]
fn interactive_auth_chooses_method_and_encrypts_secret() {
    let temp = TempDir::new().unwrap();
    let context = context(&temp);

    // An empty answer selects the documented default of password authentication.
    let default = Script::new(&[""], &["p@ssw0rd"]);
    let (auth_type, key_path, secret) =
        resolve_auth(&context, None, None, None, Some(&default), None).unwrap();
    assert_eq!(auth_type, "password");
    assert_eq!(key_path, None);
    assert_eq!(
        decrypt(&context, secret.as_deref().unwrap()).unwrap(),
        b"p@ssw0rd"
    );

    let password = Script::new(&["password"], &["p@ssw0rd"]);
    let (auth_type, _, secret) =
        resolve_auth(&context, None, None, None, Some(&password), None).unwrap();
    assert_eq!(auth_type, "password");
    assert_eq!(
        decrypt(&context, secret.as_deref().unwrap()).unwrap(),
        b"p@ssw0rd"
    );

    let key = Script::new(&["key", "/tmp/id_ed25519"], &["secret"]);
    let (auth_type, key_path, secret) =
        resolve_auth(&context, None, None, None, Some(&key), None).unwrap();
    assert_eq!(auth_type, "key");
    assert_eq!(key_path.as_deref(), Some("/tmp/id_ed25519"));
    assert_eq!(
        decrypt(&context, secret.as_deref().unwrap()).unwrap(),
        b"secret"
    );

    let empty_key = Script::new(&["key", "", "/tmp/retry-key"], &[""]);
    let (_, path, _) = resolve_auth(&context, None, None, None, Some(&empty_key), None).unwrap();
    assert_eq!(path.as_deref(), Some("/tmp/retry-key"));
    let unknown = Script::new(&["token", "password"], &["retry-password"]);
    let (method, _, _) = resolve_auth(&context, None, None, None, Some(&unknown), None).unwrap();
    assert_eq!(method, "password");
}

#[test]
fn interactive_prompt_accepts_the_configured_default_method() {
    let temp = TempDir::new().unwrap();
    let context = context(&temp);

    // `[defaults] auth = "key"` makes Enter at the method prompt choose key auth.
    let keyed = Script::new(&["", "/tmp/id_ed25519"], &["secret"]);
    let (auth_type, key_path, secret) =
        resolve_auth(&context, None, None, None, Some(&keyed), Some("key")).unwrap();
    assert_eq!(auth_type, "key");
    assert_eq!(key_path.as_deref(), Some("/tmp/id_ed25519"));
    assert_eq!(
        decrypt(&context, secret.as_deref().unwrap()).unwrap(),
        b"secret"
    );
}
