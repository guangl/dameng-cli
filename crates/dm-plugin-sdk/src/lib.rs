//! Stable process protocol and Rust entry point for `dm` plugins.
use std::{env, error::Error, ffi::OsString, path::PathBuf};

pub const API_VERSION: u32 = 1;
pub type PluginResult = Result<i32, Box<dyn Error + Send + Sync>>;

/// Inputs supplied by the host. Standard I/O and the working directory are inherited.
#[derive(Debug)]
pub struct Context {
    pub args: Vec<OsString>,
    pub plugin_dir: PathBuf,
    pub home: PathBuf,
}

impl Context {
    pub fn from_env() -> Result<Self, Box<dyn Error + Send + Sync>> {
        let version =
            env::var("DM_PLUGIN_API_VERSION").map_err(|_| "Run this plugin through dm <plugin>")?;
        if version != API_VERSION.to_string() {
            return Err(format!("Unsupported host plugin API: {version}").into());
        }
        Ok(Self {
            args: env::args_os().skip(1).collect(),
            plugin_dir: env::var_os("DM_PLUGIN_DIR")
                .ok_or("Missing DM_PLUGIN_DIR")?
                .into(),
            home: env::var_os("DM_HOME").ok_or("Missing DM_HOME")?.into(),
        })
    }
}

/// Implement this trait in a separate Rust binary crate.
pub trait Plugin {
    fn run(&self, context: Context) -> PluginResult;
}

/// Validate the protocol, run the plugin, report errors and preserve its exit code.
pub fn run(plugin: impl Plugin) -> ! {
    let result = Context::from_env().and_then(|context| plugin.run(context));
    let code = match result {
        Ok(code) => code,
        Err(error) => {
            eprintln!("dm plugin: {error}");
            1
        }
    };
    std::process::exit(code)
}
