use crate::{config_path, load_config, load_servers};
use anyhow::Result;
use dm_plugin_sdk::Context;
use dm_plugin_support::{
    config::{ConfigCommand, initialize, setting, show},
    diagnostics::DiagnosticReport,
};
pub(super) fn config(context: &Context, command: ConfigCommand) -> Result<()> {
    let path = config_path(context);
    match command {
        ConfigCommand::Init => initialize(&path, include_str!("../../config.example.toml")),
        ConfigCommand::Path => {
            println!("{}", path.display());
            Ok(())
        }
        ConfigCommand::Show { json } => {
            let config = load_config(context)?;
            show(
                &path,
                vec![
                    setting(
                        "defaults.port",
                        config.defaults.port.unwrap_or(22),
                        config.defaults.port.is_some(),
                    ),
                    setting(
                        "defaults.username",
                        config.defaults.username.as_deref(),
                        config.defaults.username.is_some(),
                    ),
                    setting(
                        "defaults.auth",
                        config.defaults.auth.as_deref().unwrap_or("password"),
                        config.defaults.auth.is_some(),
                    ),
                    setting(
                        "defaults.key",
                        config.defaults.key.as_deref(),
                        config.defaults.key.is_some(),
                    ),
                    setting(
                        "test.connect_timeout",
                        config.test.connect_timeout.unwrap_or(10),
                        config.test.connect_timeout.is_some(),
                    ),
                ],
                json,
            )
        }
    }
}
pub(super) fn doctor(context: &Context, json: bool) -> Result<i32> {
    let mut report = DiagnosticReport::default();
    match load_config(context) {
        Ok(_) => report.checks.push("配置格式有效".into()),
        Err(error) => report.issues.push(format!("配置异常：{error:#}")),
    }
    let entries = match load_servers(context) {
        Ok(entries) => {
            report
                .checks
                .push(format!("连接存储有效：{} 条连接", entries.len()));
            entries
        }
        Err(error) => {
            report.issues.push(format!("连接存储异常：{error:#}"));
            Vec::new()
        }
    };
    report.tool("ssh");
    if entries.iter().any(|server| server.auth_type == "password") {
        report.tool("sshpass");
    }
    for server in &entries {
        if server.auth_type == "key" {
            if let Some(key) = &server.key_path {
                report.key(&server.name, std::path::Path::new(key));
            } else {
                report
                    .issues
                    .push(format!("{}：未配置私钥路径", server.name));
            }
        }
    }
    report.print(json)
}
