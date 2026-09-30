use anyhow::{Context, Result};
use dm_plugin_sdk::Context as PluginContext;
use std::process::Command;

use crate::storage::config::{DEFAULT_CONNECT_TIMEOUT, load_config};
use crate::storage::crypto::decrypt;
use crate::storage::servers::load_servers;

pub fn ssh_command(context: &PluginContext, name: &str, test: bool) -> Result<Command> {
    let server = load_servers(context)?
        .into_iter()
        .find(|server| server.name == name)
        .with_context(|| format!("SSH server '{name}' is not configured"))?;
    let destination = format!("{}@{}", server.username, server.host);
    let mut common: Vec<String> = vec!["-p".to_owned(), server.port.to_string()];
    if test {
        let timeout = load_config(context)?
            .test
            .connect_timeout
            .unwrap_or(DEFAULT_CONNECT_TIMEOUT);
        common.extend([
            "-o".to_owned(),
            "BatchMode=yes".to_owned(),
            "-o".to_owned(),
            format!("ConnectTimeout={timeout}"),
        ]);
    }
    let mut command;
    match server.auth_type.as_str() {
        "key" => {
            let key_path = server
                .key_path
                .as_deref()
                .context("SSH key path is not configured")?;
            command = Command::new("ssh");
            command.args(["-i", key_path]);
            command.args(&common);
        }
        _ => {
            let password = server
                .secret
                .as_deref()
                .map(|secret| decrypt(context, secret))
                .transpose()?
                .map(String::from_utf8)
                .transpose()?
                .context("SSH password is not configured")?;
            command = Command::new("sshpass");
            command.args(["-p", &password, "ssh"]);
            command.args(&common);
        }
    }
    command.arg(&destination);
    if test {
        command.arg("true");
    }
    Ok(command)
}
