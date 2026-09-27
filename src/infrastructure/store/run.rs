use anyhow::{Context, Result};
use log::{debug, info};
use std::{ffi::OsString, fs, process::Command};

use crate::API_VERSION;

use super::{PluginStore, inherit_safe_environment};

impl PluginStore {
    pub fn run(&self, name: &str, args: &[OsString]) -> Result<i32> {
        info!("running plugin {name}");
        let (root, manifest) = self.load(name)?;
        let config_dir = self.home.join("config").join(name);
        let data_dir = self.home.join("data").join(name);
        let cache_dir = self.home.join("cache").join(name);
        for directory in [&config_dir, &data_dir, &cache_dir] {
            fs::create_dir_all(directory)?;
        }
        let home = fs::canonicalize(&self.home)?;
        let mut command = Command::new(manifest.entrypoint(&root)?);
        command.env_clear();
        inherit_safe_environment(&mut command, &manifest, &self.plugin_environment);
        let status = command
            .args(args)
            .env("DM_PLUGIN_API_VERSION", API_VERSION.to_string())
            .env("DM_PLUGIN_CAPABILITIES", "config-dirs-v1")
            .env("DM_PLUGIN_DIR", &root)
            .env("DM_PLUGIN_HOME", &home)
            // Backward-compatible alias: plugins built against the published
            // dm-plugin-sdk 0.2.0 still read DM_HOME.
            .env("DM_HOME", &home)
            .env("DM_PLUGIN_CONFIG_DIR", fs::canonicalize(config_dir)?)
            .env("DM_PLUGIN_DATA_DIR", fs::canonicalize(data_dir)?)
            .env("DM_PLUGIN_CACHE_DIR", fs::canonicalize(cache_dir)?)
            .status()
            .with_context(|| format!("Start plugin '{name}'"))?;
        let code = match status.code() {
            Some(code) => code,
            None => {
                #[cfg(unix)]
                {
                    use std::os::unix::process::ExitStatusExt;
                    status.signal().map(|signal| 128 + signal).unwrap_or(0)
                }
                #[cfg(not(unix))]
                {
                    anyhow::bail!("Plugin terminated without an exit code")
                }
            }
        };
        debug!("plugin {name} exited with code {code}");
        Ok(code)
    }
}
