use super::fields::Fields;
use crate::{Prompter, Server, encrypt, load_servers, resolve_auth, upsert_server};
use anyhow::{Context as _, Result, ensure};
use dm_plugin_sdk::Context;
use dm_plugin_support::interaction::{confirm, edit_field, port};

pub(super) fn edit(
    context: &Context,
    name: &str,
    fields: Fields,
    prompter: Option<&dyn Prompter>,
) -> Result<()> {
    let old = load_servers(context)?
        .into_iter()
        .find(|server| server.name == name)
        .context("SSH 连接不存在，请使用 dm ssh list 查看连接")?;
    let host = edit_field(fields.host, &old.host, "地址", prompter)?;
    let username = edit_field(fields.username, &old.username, "用户名", prompter)?;
    let port = port(fields.port, old.port, prompter)?;
    let same_key = fields.password.is_none()
        && fields.passphrase.is_none()
        && old.auth_type == "key"
        && fields.key.as_ref().is_some_and(|key| {
            old.key_path
                .as_deref()
                .is_some_and(|old| key == std::path::Path::new(old))
        });
    let (auth_type, key_path, secret) = if same_key {
        (old.auth_type, old.key_path, old.secret)
    } else if fields.password.is_some() || fields.key.is_some() {
        // Authentication changes never reuse a password as a key passphrase.
        resolve_auth(
            context,
            fields.password,
            fields.key,
            fields.passphrase,
            None,
            None,
        )?
    } else if let Some(passphrase) = fields.passphrase {
        ensure!(old.auth_type == "key", "--passphrase 仅用于密钥认证");
        let secret = if passphrase.is_empty() {
            None
        } else {
            Some(encrypt(context, passphrase.as_bytes())?)
        };
        (old.auth_type, old.key_path, secret)
    } else {
        (old.auth_type, old.key_path, old.secret)
    };
    if prompter.is_some() {
        confirm(
            prompter,
            fields.yes,
            &format!("保存 {name}：{username}@{host}:{port}，认证 {auth_type}（秘密已隐藏）？"),
        )?;
    }
    upsert_server(
        context,
        &Server {
            name: name.into(),
            host,
            port,
            username,
            auth_type,
            key_path,
            secret,
        },
    )?;
    println!("已修改 SSH 连接 {name}。下一步：dm ssh test {name}");
    Ok(())
}
