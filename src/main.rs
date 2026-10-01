use dameng_cli::{
    Config, DEFAULT_LOG_FILTER, cleanup_self_update_backup, cli, home_from_env,
    log_filter_from_env, logging,
};
use std::path::Path;

fn main() {
    // Tab completion is quiet, read-only, and works even with invalid settings.
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "complete")
    {
        let config = Config::from_env().unwrap_or_default();
        let _ = cli::run(&config);
        return;
    }
    // Diagnostics are written next to the plugin store, so the data directory is
    // resolved before anything can be logged.
    let home = home_from_env().ok();
    // The configuration file also selects the log filter, so it is loaded before
    // the logging backend starts.
    let raw_args: Vec<_> = std::env::args_os().collect();
    let config_discovery = raw_args.get(1).is_some_and(|arg| arg == "config")
        && raw_args
            .get(2)
            .is_some_and(|arg| arg == "path" || arg == "init");
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(_) if config_discovery => Config::default(),
        Err(error) => {
            // Only the environment can decide now that the file is unusable;
            // honouring it keeps `DM_LOG=off` from creating a log file.
            let filter = log_filter_from_env().unwrap_or_else(|| DEFAULT_LOG_FILTER.to_owned());
            let _ = init_logging(&Config::default(), &filter, home.as_deref());
            cli::report(&error);
            std::process::exit(1);
        }
    };
    if let Err(error) = init_logging(&config, &config.log_filter(), home.as_deref()) {
        cli::report(&error);
        std::process::exit(1);
    }
    let _ = cleanup_self_update_backup();
    let code = match cli::run(&config) {
        Ok(code) => code,
        Err(error) => {
            cli::report(&error);
            1
        }
    };
    std::process::exit(code);
}

/// Initialize file-only diagnostics; a missing destination disables logging.
fn init_logging(config: &Config, filter: &str, home: Option<&Path>) -> anyhow::Result<()> {
    match home {
        Some(home) => {
            logging::init_at(filter, &config.log_directory(home), config.log_max_bytes()?)
        }
        None => logging::init_disabled(),
    }
    Ok(())
}
