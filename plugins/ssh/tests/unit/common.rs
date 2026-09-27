use std::ffi::OsString;
use std::fs;

use dm_plugin_sdk::Context as PluginContext;
use tempfile::TempDir;

#[allow(dead_code)]
pub fn context(temp: &TempDir) -> PluginContext {
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    PluginContext {
        args: vec![OsString::from("ssh")],
        plugin_dir: temp.path().join("plugin"),
        home,
        config_dir: temp.path().join("config/ssh"),
        data_dir: temp.path().join("data/ssh"),
        cache_dir: temp.path().join("cache/ssh"),
        capabilities: vec!["config-dirs-v1".to_owned()],
    }
}

/// Scripted answers for the interactive branches, which a test process cannot
/// drive through a real terminal.
pub struct Script {
    lines: std::cell::RefCell<std::collections::VecDeque<String>>,
    secrets: std::cell::RefCell<std::collections::VecDeque<String>>,
}

impl Script {
    pub fn new(lines: &[&str], secrets: &[&str]) -> Self {
        Self {
            lines: std::cell::RefCell::new(lines.iter().map(|line| (*line).to_owned()).collect()),
            secrets: std::cell::RefCell::new(
                secrets.iter().map(|secret| (*secret).to_owned()).collect(),
            ),
        }
    }

    fn take(queue: &std::cell::RefCell<std::collections::VecDeque<String>>) -> String {
        queue
            .borrow_mut()
            .pop_front()
            .expect("script ran out of answers")
    }
}

impl dm_plugin_ssh::Prompter for Script {
    fn line(&self, prompt: &str) -> anyhow::Result<String> {
        assert!(!prompt.is_empty(), "prompts must be visible text");
        Ok(Self::take(&self.lines))
    }

    fn secret(&self, prompt: &str) -> anyhow::Result<String> {
        assert!(!prompt.is_empty(), "prompts must be visible text");
        Ok(Self::take(&self.secrets))
    }
}
