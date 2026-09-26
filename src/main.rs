mod cli;

use dameng_cli::cleanup_self_update_backup;

fn main() {
    init_logging();
    let _ = cleanup_self_update_backup();
    let code = match cli::run() {
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
/// Logs always go to stderr so stdout stays machine-readable. The default
/// filter is `info`; set `DM_LOG` (or `RUST_LOG`) to change it, for example
/// `DM_LOG=debug` or `DM_LOG=dm=debug`.
fn init_logging() {
    // `DM_LOG` takes precedence, then `RUST_LOG`, then the default `info`.
    let filter_var = if std::env::var("DM_LOG").is_ok() {
        "DM_LOG"
    } else {
        "RUST_LOG"
    };
    let env = env_logger::Env::new().filter_or(filter_var, "info");
    env_logger::Builder::from_env(env)
        .format_timestamp_secs()
        .format_target(false)
        .target(env_logger::Target::Stderr)
        .init();
}
