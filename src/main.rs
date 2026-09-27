use dameng_cli::{
    Config, DEFAULT_LOG_FILTER, cleanup_self_update_backup, cli, home_from_env, logging,
};
use std::path::Path;

fn main() {
    // Diagnostics are written next to the plugin store, so the data directory is
    // resolved before anything can be logged.
    let home = home_from_env().ok();
    // The configuration file also selects the log filter, so it is loaded before
    // the logging backend starts.
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(error) => {
            init_logging(DEFAULT_LOG_FILTER, home.as_deref());
            cli::report(&error);
            std::process::exit(1);
        }
    };
    init_logging(&config.log_filter(), home.as_deref());
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

/// Configure the logging backend.
///
/// Logs go to `<DM_PLUGIN_HOME>/dm.log` so stdout stays machine-readable and
/// stderr keeps only the progress bar, plugin output and the user-facing report;
/// without a usable data directory they fall back to stderr. The filter comes
/// from `DM_LOG`, then `RUST_LOG`, then `<DM_PLUGIN_HOME>/config.toml`, and
/// defaults to `info`.
fn init_logging(filter: &str, home: Option<&Path>) {
    match home {
        Some(home) => logging::init(filter, home),
        None => logging::init_stderr(filter),
    }
}
