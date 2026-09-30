use crate::{config_path, load_config, load_connections};
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
                        config.defaults.port.unwrap_or(5236),
                        config.defaults.port.is_some(),
                    ),
                    setting(
                        "defaults.username",
                        config.defaults.username.as_deref().unwrap_or("SYSDBA"),
                        config.defaults.username.is_some(),
                    ),
                    setting(
                        "defaults.driver",
                        config
                            .defaults
                            .driver
                            .as_deref()
                            .unwrap_or("DM8 ODBC DRIVER"),
                        config.defaults.driver.is_some(),
                    ),
                    setting(
                        "defaults.schema",
                        config.defaults.schema.as_deref(),
                        config.defaults.schema.is_some(),
                    ),
                    setting(
                        "connect.timeout",
                        config.connect.timeout.unwrap_or(10),
                        config.connect.timeout.is_some(),
                    ),
                    setting(
                        "connect.probe",
                        config.connect.probe.as_deref().unwrap_or("SELECT 1"),
                        config.connect.probe.is_some(),
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
    let _entries = match load_connections(context) {
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
    report
        .issues
        .push("数据库驱动尚未实现；目前支持连接配置管理，test/exec 暂不可用".into());
    report.print(json)
}
