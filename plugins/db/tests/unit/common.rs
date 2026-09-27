//! Helpers shared by the unit tests: plugin contexts, sample connections and
//! the scripted prompter.

use std::{cell::RefCell, collections::VecDeque, ffi::OsString, fs};

use dm_plugin_db::{DatabaseConnection, Prompter, encrypt, upsert_connection};
use dm_plugin_sdk::Context as PluginContext;
use tempfile::TempDir;

pub(crate) fn context(temp: &TempDir) -> PluginContext {
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    PluginContext {
        args: vec![OsString::from("db")],
        plugin_dir: temp.path().join("plugin"),
        home,
        config_dir: temp.path().join("config/db"),
        data_dir: temp.path().join("data/db"),
        cache_dir: temp.path().join("cache/db"),
        capabilities: vec!["config-dirs-v1".to_owned()],
    }
}

pub(crate) fn sample(name: &str) -> DatabaseConnection {
    DatabaseConnection {
        name: name.to_owned(),
        host: "10.0.0.8".to_owned(),
        port: 5236,
        username: "SYSDBA".to_owned(),
        schema: None,
        driver: "DM8 ODBC DRIVER".to_owned(),
        secret: None,
    }
}

pub(crate) fn with_args(context: &mut PluginContext, args: &[&str]) {
    context.args = args.iter().map(OsString::from).collect();
}

pub(crate) fn stored_connection(context: &PluginContext, name: &str, schema: Option<&str>) {
    let mut connection = sample(name);
    connection.schema = schema.map(str::to_owned);
    connection.secret = Some(encrypt(context, b"SYSDBA@123").unwrap());
    upsert_connection(context, &connection).unwrap();
}

/// Scripted answers for the interactive branches, which a test process cannot
/// drive through a real terminal.
pub(crate) struct Script {
    lines: RefCell<VecDeque<String>>,
    secrets: RefCell<VecDeque<String>>,
}

impl Script {
    pub(crate) fn new(lines: &[&str], secrets: &[&str]) -> Self {
        Self {
            lines: RefCell::new(lines.iter().map(|line| (*line).to_owned()).collect()),
            secrets: RefCell::new(secrets.iter().map(|secret| (*secret).to_owned()).collect()),
        }
    }

    /// Next scripted answer; an exhausted script fails like a closed terminal.
    fn take(queue: &RefCell<VecDeque<String>>) -> anyhow::Result<String> {
        queue
            .borrow_mut()
            .pop_front()
            .ok_or_else(|| anyhow::anyhow!("terminal closed"))
    }
}

impl Prompter for Script {
    fn line(&self, prompt: &str) -> anyhow::Result<String> {
        assert!(!prompt.is_empty(), "prompts must be visible text");
        Self::take(&self.lines)
    }

    fn secret(&self, prompt: &str) -> anyhow::Result<String> {
        assert!(!prompt.is_empty(), "prompts must be visible text");
        Self::take(&self.secrets)
    }
}
