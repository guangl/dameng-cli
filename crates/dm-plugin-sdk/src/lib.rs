//! Stable process protocol and Rust entry point for `dm` plugins.
use std::{env, error::Error, ffi::OsString, path::PathBuf};

pub const API_VERSION: u32 = 1;
pub const CAPABILITY_CONFIG_DIRS: &str = "config-dirs-v1";

/// Conventional name of a plugin's own configuration file inside its config directory.
pub const CONFIG_FILE: &str = "config.toml";

pub type PluginResult = Result<i32, Box<dyn Error + Send + Sync>>;

/// Inputs supplied by the host. Standard I/O and the working directory are inherited.
#[derive(Debug)]
pub struct Context {
    pub args: Vec<OsString>,
    pub plugin_dir: PathBuf,
    pub home: PathBuf,
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub capabilities: Vec<String>,
}

impl Context {
    /// Path of this plugin's own configuration file.
    ///
    /// The host creates the directory and passes it as `DM_PLUGIN_CONFIG_DIR`;
    /// the file inside it belongs to the plugin, which owns its schema. The host
    /// never reads or rewrites it, so plugins stay configurable without adding
    /// keys to the host configuration file.
    pub fn config_file(&self) -> PathBuf {
        self.config_dir.join(CONFIG_FILE)
    }

    pub fn from_env() -> Result<Self, Box<dyn Error + Send + Sync>> {
        let version =
            env::var("DM_PLUGIN_API_VERSION").map_err(|_| "Run this plugin through dm <plugin>")?;
        if version != API_VERSION.to_string() {
            return Err(format!("Unsupported host plugin API: {version}").into());
        }
        let capabilities = env::var("DM_PLUGIN_CAPABILITIES")
            .unwrap_or_default()
            .split(',')
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if !capabilities
            .iter()
            .any(|value| value == CAPABILITY_CONFIG_DIRS)
        {
            return Err("Host does not provide per-plugin directories".into());
        }
        Ok(Self {
            args: env::args_os().skip(1).collect(),
            plugin_dir: env::var_os("DM_PLUGIN_DIR")
                .ok_or("Missing DM_PLUGIN_DIR")?
                .into(),
            home: env::var_os("DM_PLUGIN_HOME")
                .ok_or("Missing DM_PLUGIN_HOME")?
                .into(),
            config_dir: required_path("DM_PLUGIN_CONFIG_DIR")?,
            data_dir: required_path("DM_PLUGIN_DATA_DIR")?,
            cache_dir: required_path("DM_PLUGIN_CACHE_DIR")?,
            capabilities,
        })
    }
}

/// Read a required environment path. Public for integration tests.
#[doc(hidden)]
pub fn required_path(name: &str) -> Result<PathBuf, Box<dyn Error + Send + Sync>> {
    env::var_os(name)
        .map(PathBuf::from)
        .ok_or_else(|| format!("Missing {name}").into())
}

/// Implement this trait in a separate Rust binary crate.
pub trait Plugin {
    fn run(&self, context: Context) -> PluginResult;
}

/// Validate the protocol, run the plugin, report errors and preserve its exit code.
pub fn run(plugin: impl Plugin) -> ! {
    std::process::exit(run_code(plugin));
}

/// Testable core of `run`. Public for integration tests.
#[doc(hidden)]
pub fn run_code(plugin: impl Plugin) -> i32 {
    let result = Context::from_env().and_then(|context| plugin.run(context));
    match result {
        Ok(code) => code,
        Err(error) => {
            eprintln!("dm plugin: {error}");
            1
        }
    }
}
