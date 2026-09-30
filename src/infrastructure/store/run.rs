use anyhow::{Context, Result};
use log::{debug, info};
use std::{
    ffi::OsString,
    fs,
    process::{Command, Stdio},
};

use crate::API_VERSION;

use super::{PluginStore, inherit_safe_environment};

impl PluginStore {
    pub fn run(&self, name: &str, args: &[OsString]) -> Result<i32> {
        info!("running plugin {name}");
        let mut command = self.plugin_command(name, true)?;
        let status = command
            .args(args)
            .status()
            .with_context(|| format!("Start plugin '{name}'"))?;
        exit_code(status)
    }

    pub(crate) fn plugin_command(&self, name: &str, create: bool) -> Result<Command> {
        let (root, manifest) = if create {
            self.load(name)?
        } else {
            self.load_completion(name)?
        };
        let config_dir = self.home.join("config").join(name);
        let data_dir = self.home.join("data").join(name);
        let cache_dir = self.home.join("cache").join(name);
        if create {
            for directory in [&config_dir, &data_dir, &cache_dir] {
                fs::create_dir_all(directory)?;
            }
        }
        let home = fs::canonicalize(&self.home)?;
        let mut command = Command::new(manifest.entrypoint(&root)?);
        command.env_clear();
        inherit_safe_environment(&mut command, &manifest, &self.plugin_environment);
        command
            .env("DM_PLUGIN_API_VERSION", API_VERSION.to_string())
            .env("DM_PLUGIN_CAPABILITIES", "config-dirs-v1,completion-v1")
            .env("DM_PLUGIN_DIR", &root)
            .env("DM_PLUGIN_HOME", &home)
            .env("DM_HOME", &home)
            .env(
                "DM_PLUGIN_CONFIG_DIR",
                fs::canonicalize(&config_dir).unwrap_or(config_dir),
            )
            .env(
                "DM_PLUGIN_DATA_DIR",
                fs::canonicalize(&data_dir).unwrap_or(data_dir),
            )
            .env(
                "DM_PLUGIN_CACHE_DIR",
                fs::canonicalize(&cache_dir).unwrap_or(cache_dir),
            );
        Ok(command)
    }

    /// Completion runs with no stdin and a deadline; unsupported plugins stay silent.
    pub fn complete_plugin(&self, name: &str, words: &[String]) -> Result<Vec<String>> {
        if !self.load_completion(name)?.1.completion {
            return Ok(Vec::new());
        }
        let mut command = self.plugin_command(name, false)?;
        let output = tempfile::tempfile()?;
        let mut child = command
            .arg("__complete")
            .args(words)
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .stdout(output.try_clone()?)
            .spawn()?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(1000);
        loop {
            if let Some(status) = child.try_wait()? {
                if !status.success() {
                    return Ok(Vec::new());
                }
                break;
            }
            if std::time::Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Ok(Vec::new());
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        use std::io::{Read, Seek};
        let mut output = output;
        output.rewind()?;
        let mut text = String::new();
        output.take(64 * 1024).read_to_string(&mut text)?;
        Ok(text
            .lines()
            .take(1000)
            .filter(|line| !line.is_empty() && !line.chars().any(char::is_control))
            .map(str::to_owned)
            .collect())
    }
}

fn exit_code(status: std::process::ExitStatus) -> Result<i32> {
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
    debug!("plugin exited with code {code}");
    Ok(code)
}
