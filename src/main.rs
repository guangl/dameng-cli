mod cli;

use dameng_cli::cleanup_self_update_backup;

fn main() {
    let _ = cleanup_self_update_backup();
    let code = match cli::run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("dm: {error:#}");
            1
        }
    };
    std::process::exit(code);
}
