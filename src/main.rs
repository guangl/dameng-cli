mod cli;

use dameng_cli::{Config, DEFAULT_LOG_FILTER, cleanup_self_update_backup};

fn main() {
    // The configuration file also selects the log filter, so it is loaded before
    // the logging backend starts.
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(error) => {
            init_logging(DEFAULT_LOG_FILTER);
            cli::report(&error);
            std::process::exit(1);
        }
    };
    init_logging(&config.log_filter());
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
/// Logs always go to stderr so stdout stays machine-readable. The filter comes
/// from `DM_LOG`, then `RUST_LOG`, then `<DM_PLUGIN_HOME>/config.toml`, and
/// defaults to `info`.
fn init_logging(filter: &str) {
    env_logger::Builder::new()
        .parse_filters(filter)
        .format_timestamp_secs()
        .format_target(false)
        .target(env_logger::Target::Stderr)
        .init();
}
