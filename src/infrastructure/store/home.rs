use anyhow::{Context, Result, ensure};
use log::debug;
use rusqlite::Connection;
use std::{env, fs, path::PathBuf};

/// Resolve the host data directory from the environment.
///
/// `DM_PLUGIN_HOME` wins; otherwise the platform default is used and relative
/// paths are resolved against the current working directory. The host log file
/// and the configuration file live here as well, so the entry point resolves the
/// directory before the logging backend starts.
pub fn home_from_env() -> Result<PathBuf> {
    let home = if let Some(home) = env::var_os("DM_PLUGIN_HOME") {
        ensure!(!home.is_empty(), "DM_PLUGIN_HOME must not be empty");
        PathBuf::from(home)
    } else {
        #[cfg(windows)]
        {
            PathBuf::from(
                env::var_os("LOCALAPPDATA").context("Set DM_PLUGIN_HOME or LOCALAPPDATA")?,
            )
            .join("dm")
        }
        #[cfg(not(windows))]
        {
            PathBuf::from(env::var_os("HOME").context("Set DM_PLUGIN_HOME or HOME")?)
                .join(".config/dm")
        }
    };
    Ok(if home.is_absolute() {
        home
    } else {
        env::current_dir()?.join(home)
    })
}

/// An explicit store path makes embedding and tests independent of user state.
pub struct PluginStore {
    pub(crate) home: PathBuf,
    /// Progress-bar override from the configuration file; `None` follows stderr.
    progress: Option<bool>,
    /// Extra environment variable names inherited by plugins and hooks.
    pub(crate) plugin_environment: Vec<String>,
}

impl PluginStore {
    pub fn new(home: impl Into<PathBuf>) -> Self {
        Self {
            home: home.into(),
            progress: None,
            plugin_environment: Vec::new(),
        }
    }

    /// Per-plugin directories: configuration, data and cache.
    pub fn plugin_directories(&self, name: &str) -> [PathBuf; 3] {
        self.per_plugin_directories(name)
    }

    /// Apply the `progress` configuration key; `None` keeps following stderr.
    pub fn with_progress(mut self, progress: Option<bool>) -> Self {
        self.progress = progress;
        self
    }

    /// Apply the `plugin_environment` configuration key.
    pub fn with_plugin_environment(mut self, plugin_environment: Vec<String>) -> Self {
        self.plugin_environment = plugin_environment;
        self
    }

    /// Whether progress bars should be drawn for this run.
    #[doc(hidden)]
    pub fn progress_enabled(&self) -> bool {
        use std::io::IsTerminal;
        self.progress
            .unwrap_or_else(|| std::io::stderr().is_terminal())
    }

    pub fn from_env() -> Result<Self> {
        Ok(Self::new(home_from_env()?))
    }

    pub(crate) fn plugins(&self) -> PathBuf {
        self.home.join("plugins")
    }

    pub(crate) fn backups(&self) -> PathBuf {
        self.home.join("backups")
    }

    pub(crate) fn per_plugin_directories(&self, name: &str) -> [PathBuf; 3] {
        [
            self.home.join("config").join(name),
            self.home.join("data").join(name),
            self.home.join("cache").join(name),
        ]
    }

    fn database(&self) -> PathBuf {
        self.home.join("store.sqlite3")
    }

    fn ensure_home_writable(&self) -> Result<()> {
        tempfile::Builder::new()
            .prefix(".dm-write-probe-")
            .tempfile_in(&self.home)
            .with_context(|| {
                format!(
                    "DM_PLUGIN_HOME directory {} is not writable; check permissions or set DM_PLUGIN_HOME to a writable directory",
                    self.home.display()
                )
            })?;
        Ok(())
    }

    fn store_open_error(&self, error: rusqlite::Error) -> anyhow::Error {
        if error.sqlite_error_code() == Some(rusqlite::ErrorCode::CannotOpen) {
            anyhow::anyhow!(
                "Cannot open SQLite plugin store at {}; make sure the DM_PLUGIN_HOME directory is writable: {error}",
                self.database().display()
            )
        } else {
            anyhow::Error::from(error)
        }
    }

    pub(crate) fn connect(&self) -> Result<Connection> {
        fs::create_dir_all(&self.home).context("Create dm data directory")?;
        self.ensure_home_writable()?;
        let connection = Connection::open(self.database())
            .map_err(|error| self.store_open_error(error))
            .context("Open SQLite plugin store")?;
        debug!("opened plugin store {}", self.database().display());
        connection
            .busy_timeout(std::time::Duration::from_secs(5))
            .context("Configure SQLite plugin store")?;
        connection
            .execute_batch(
                "PRAGMA journal_mode = WAL;
                 PRAGMA foreign_keys = ON;
                 PRAGMA user_version = 3;
                 CREATE TABLE IF NOT EXISTS installed_plugins (
                     name TEXT PRIMARY KEY,
                     manifest TEXT NOT NULL,
                     installed_at INTEGER NOT NULL DEFAULT (unixepoch()),
                     source TEXT,
                     revision TEXT,
                     source_ref TEXT,
                     checksum TEXT NOT NULL DEFAULT ''
                 ) STRICT;",
            )
            .map_err(|error| self.store_open_error(error))
            .context("Initialize SQLite plugin store")?;
        Ok(connection)
    }
}
