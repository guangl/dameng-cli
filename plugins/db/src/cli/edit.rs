use super::fields::Fields;
use crate::{DatabaseConnection, Prompter, encrypt, find_connection, upsert_connection};
use anyhow::Result;
use dm_plugin_sdk::Context;
use dm_plugin_support::interaction::{confirm, edit_field, port};

pub(super) fn edit(
    context: &Context,
    name: &str,
    fields: Fields,
    prompter: Option<&dyn Prompter>,
) -> Result<()> {
    let old = find_connection(context, name)?;
    let host = edit_field(fields.host, &old.host, "地址", prompter)?;
    let username = edit_field(fields.username, &old.username, "用户名", prompter)?;
    let driver = edit_field(fields.driver, &old.driver, "驱动", prompter)?;
    let port = port(fields.port, old.port, prompter)?;
    let schema = if fields.clear_schema {
        None
    } else if let Some(schema) = fields.schema {
        Some(schema)
    } else if let Some(p) = prompter {
        let value = p.line(&format!(
            "Schema [{}]（回车保留；--clear-schema 清除）: ",
            old.schema.as_deref().unwrap_or("")
        ))?;
        if value.is_empty() {
            old.schema
        } else {
            Some(value)
        }
    } else {
        old.schema
    };
    // Passwords are kept encrypted unless a replacement was explicitly supplied.
    let secret = match fields.password {
        Some(password) => Some(encrypt(
            context,
            crate::resolve_password(Some(password), None)?.as_bytes(),
        )?),
        None => old.secret,
    };
    if prompter.is_some() {
        confirm(
            prompter,
            fields.yes,
            &format!("保存 {name}：{username}@{host}:{port}（密码已隐藏）？"),
        )?;
    }
    upsert_connection(
        context,
        &DatabaseConnection {
            name: name.into(),
            host,
            port,
            username,
            schema,
            driver,
            secret,
        },
    )?;
    println!("已修改数据库连接 {name}。运行 dm db list 查看连接。");
    Ok(())
}
