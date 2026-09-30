use crate::{Config, home_from_env};
use anyhow::Result;
use dm_plugin_support::config::{ConfigCommand, Setting, initialize, setting, show};

pub(super) fn run(config: &Config, command: ConfigCommand) -> Result<()> {
    let path = Config::path_in(&home_from_env()?);
    match command {
        ConfigCommand::Init => initialize(&path, include_str!("../../examples/config.toml")),
        ConfigCommand::Path => {
            println!("{}", path.display());
            Ok(())
        }
        ConfigCommand::Show { json } => {
            let environment_configured = std::fs::read_to_string(&path)
                .ok()
                .and_then(|text| toml::from_str::<toml::Value>(&text).ok())
                .is_some_and(|value| {
                    value
                        .get("plugin")
                        .and_then(|table| table.get("environment"))
                        .is_some()
                });
            let settings = vec![
                effective(
                    setting("log.level", config.log_filter(), config.log.level.is_some()),
                    &["DM_LOG", "RUST_LOG"],
                ),
                effective(
                    setting(
                        "log.directory",
                        config.log_directory(&home_from_env()?),
                        config.log.directory.is_some(),
                    ),
                    &["DM_LOG_DIR"],
                ),
                effective(
                    setting(
                        "log.max_size_mb",
                        config.log_max_bytes()? / (1024 * 1024),
                        config.log.max_size_mb.is_some(),
                    ),
                    &["DM_LOG_MAX_SIZE_MB"],
                ),
                effective(
                    setting(
                        "update.repository",
                        config
                            .update_repository()
                            .unwrap_or_else(|| "guangl/dameng-cli".into()),
                        config.update.repository.is_some(),
                    ),
                    &["DM_UPDATE_REPOSITORY"],
                ),
                effective(
                    setting(
                        "update.target",
                        config
                            .update_target()
                            .unwrap_or_else(|| env!("DM_HOST_TARGET").into()),
                        config.update.target.is_some(),
                    ),
                    &["DM_UPDATE_TARGET"],
                ),
                effective(
                    setting(
                        "output.progress",
                        config.progress()?.unwrap_or(true),
                        config.output.progress.is_some(),
                    ),
                    &["DM_PROGRESS"],
                ),
                effective(
                    setting(
                        "plugin.environment",
                        config.plugin_environment()?,
                        environment_configured,
                    ),
                    &["DM_PLUGIN_ENVIRONMENT"],
                ),
            ];
            show(&path, settings, json)
        }
    }
}
fn effective(mut setting: Setting, variables: &[&str]) -> Setting {
    for variable in variables {
        if std::env::var(variable)
            .ok()
            .is_some_and(|value| setting.key == "log.level" || !value.trim().is_empty())
        {
            setting.source = format!("env:{variable}");
            break;
        }
    }
    setting
}
