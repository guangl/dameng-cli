use crate::{API_VERSION, Manifest, plugin::manifest::validate_name};
use anyhow::{Context, Result, bail, ensure};
use indicatif::ProgressBar;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    env,
    ffi::OsString,
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PluginInfo {
    pub manifest: Manifest,
    pub source: Option<String>,
    pub revision: Option<String>,
    pub source_ref: Option<String>,
    pub checksum: String,
    pub installed_at: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DoctorReport {
    pub issues: Vec<String>,
    pub repairs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct UpdateStatus {
    pub name: String,
    pub installed_version: String,
    pub available_version: Option<String>,
    pub update_available: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum InstallMode {
    New,
    Update,
}

/// An explicit store path makes embedding and tests independent of user state.
pub struct PluginStore {
    home: PathBuf,
}

impl PluginStore {
    pub fn new(home: impl Into<PathBuf>) -> Self {
        Self { home: home.into() }
    }

    pub fn from_env() -> Result<Self> {
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
        Ok(Self::new(if home.is_absolute() {
            home
        } else {
            env::current_dir()?.join(home)
        }))
    }

    fn plugins(&self) -> PathBuf {
        self.home.join("plugins")
    }

    fn backups(&self) -> PathBuf {
        self.home.join("backups")
    }

    fn per_plugin_directories(&self, name: &str) -> [PathBuf; 3] {
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

    fn connect(&self) -> Result<Connection> {
        fs::create_dir_all(&self.home).context("Create dm data directory")?;
        self.ensure_home_writable()?;
        let connection = Connection::open(self.database())
            .map_err(|error| self.store_open_error(error))
            .context("Open SQLite plugin store")?;
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
                 ) STRICT;
                 DROP TABLE IF EXISTS ssh_servers;",
            )
            .map_err(|error| self.store_open_error(error))
            .context("Initialize SQLite plugin store")?;
        connection
            .execute_batch(dm_plugin_sdk::SSH_SERVERS_TABLE_SCHEMA)
            .map_err(|error| self.store_open_error(error))
            .context("Initialize shared SSH servers table")?;
        Ok(connection)
    }

    pub fn install(&self, source: &str) -> Result<Manifest> {
        self.install_with_revision(source, None)
    }

    pub fn install_with_revision(&self, source: &str, revision: Option<&str>) -> Result<Manifest> {
        if Path::new(source).is_dir() {
            ensure!(
                revision.is_none(),
                "--rev is only supported for HTTPS Git sources"
            );
            let canonical = fs::canonicalize(source)?;
            return self.install_directory(
                &canonical,
                Some(canonical.display().to_string()),
                None,
                None,
                InstallMode::New,
            );
        }
        ensure!(
            source.starts_with("https://") && source.len() > 8,
            "Source must be a local plugin directory or HTTPS Git repository URL"
        );
        let (checkout, destination, resolved_revision) = checkout_git(source, revision)?;
        let result = self.install_directory(
            &destination,
            Some(source.to_owned()),
            Some(resolved_revision),
            revision.map(str::to_owned),
            InstallMode::New,
        );
        drop(checkout);
        result
    }

    fn install_directory(
        &self,
        source: &Path,
        recorded_source: Option<String>,
        revision: Option<String>,
        source_ref: Option<String>,
        mode: InstallMode,
    ) -> Result<Manifest> {
        let source = fs::canonicalize(source)?;
        let manifest = Manifest::read(&source)?;
        fs::create_dir_all(self.plugins())?;
        let plugins = fs::canonicalize(self.plugins())?;
        ensure!(
            !plugins.starts_with(&source),
            "Plugin source must not contain the plugin store"
        );
        let destination = plugins.join(&manifest.name);
        let mut connection = self.connect()?;
        let installed = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM installed_plugins WHERE name = ?1)",
            [&manifest.name],
            |row| row.get::<_, bool>(0),
        )?;
        match mode {
            InstallMode::New => {
                ensure!(
                    !installed,
                    "Plugin '{}' is already installed; use dm update",
                    manifest.name
                );
                ensure!(
                    fs::symlink_metadata(&destination)
                        .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound),
                    "Plugin '{}' already exists on disk; run dm doctor",
                    manifest.name
                );
            }
            InstallMode::Update => {
                ensure!(installed, "Plugin '{}' is not installed", manifest.name);
                ensure!(
                    fs::symlink_metadata(&destination).is_ok_and(|metadata| metadata.is_dir()),
                    "Installed plugin '{}' is missing or invalid; run dm doctor",
                    manifest.name
                );
            }
        }
        let stage = tempfile::Builder::new()
            .prefix(".install-")
            .tempdir_in(&plugins)?;
        let package = stage.path().join("package");
        let prebuilt = stage.path().join("prebuilt");
        let local_binary = source.join(manifest.executable_name());
        if local_binary.is_file() {
            fs::copy(&local_binary, &prebuilt).context("Copy local prebuilt plugin")?;
        } else {
            try_download_prebuilt(
                recorded_source.as_deref(),
                &manifest,
                source_ref.as_deref(),
                &prebuilt,
            )
            .context("Source builds are disabled; install a prebuilt plugin release")?;
        }
        if let Some(hook) = manifest.hooks.pre_install.as_deref() {
            self.run_hook(&source, hook, "pre-install", &manifest)?;
        }
        fs::create_dir(&package)?;
        fs::copy(&prebuilt, package.join(manifest.executable_name()))
            .context("Copy plugin binary")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                package.join(manifest.executable_name()),
                fs::Permissions::from_mode(0o755),
            )?;
        }
        fs::write(
            package.join(crate::MANIFEST_FILE),
            toml::to_string(&manifest)?,
        )?;
        manifest.entrypoint(&package)?;
        self.copy_hooks(&source, &package, &manifest)?;
        let checksum = sha256_file(&package.join(manifest.executable_name()))?;
        let previous = stage.path().join("previous");
        if mode == InstallMode::Update {
            fs::rename(&destination, &previous).context("Stage previous plugin version")?;
        }
        if let Err(error) = fs::rename(&package, &destination) {
            if mode == InstallMode::Update {
                let _ = fs::rename(&previous, &destination);
            }
            return Err(error).context("Publish installed plugin");
        }
        if let Some(hook) = manifest.hooks.post_install.as_deref() {
            if let Err(error) = self.run_hook(&destination, hook, "post-install", &manifest) {
                let _ = fs::remove_dir_all(&destination);
                if mode == InstallMode::Update {
                    let _ = fs::rename(&previous, &destination);
                }
                return Err(error).context("Run post-install hook");
            }
        }
        let transaction = connection.transaction()?;
        let database_result = match mode {
            InstallMode::New => transaction.execute(
                "INSERT INTO installed_plugins
                 (name, manifest, source, revision, source_ref, checksum)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    manifest.name,
                    toml::to_string(&manifest)?,
                    recorded_source,
                    revision,
                    source_ref,
                    checksum
                ],
            ),
            InstallMode::Update => transaction.execute(
                "UPDATE installed_plugins
                 SET manifest = ?2, source = ?3, revision = ?4, source_ref = ?5, checksum = ?6,
                     installed_at = unixepoch()
                 WHERE name = ?1",
                params![
                    manifest.name,
                    toml::to_string(&manifest)?,
                    recorded_source,
                    revision,
                    source_ref,
                    checksum
                ],
            ),
        };
        if let Err(error) = database_result.and_then(|_| transaction.commit()) {
            let _ = fs::remove_dir_all(&destination);
            if mode == InstallMode::Update {
                let _ = fs::rename(&previous, &destination);
            }
            return Err(error).context("Record installed plugin in SQLite");
        }
        Ok(manifest)
    }

    pub fn list(&self) -> Result<Vec<Manifest>> {
        let connection = self.connect()?;
        let names = {
            let mut statement =
                connection.prepare("SELECT name FROM installed_plugins ORDER BY name")?;
            statement
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        names
            .into_iter()
            .map(|name| self.load(&name).map(|(_, manifest)| manifest))
            .collect()
    }

    pub fn list_info(&self) -> Result<Vec<PluginInfo>> {
        let connection = self.connect()?;
        let mut statement =
            connection.prepare("SELECT name FROM installed_plugins ORDER BY name")?;
        let names = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        names.into_iter().map(|name| self.info(&name)).collect()
    }

    pub fn info(&self, name: &str) -> Result<PluginInfo> {
        validate_name(name)?;
        let values = self
            .connect()?
            .query_row(
                "SELECT manifest, source, revision, source_ref, checksum, installed_at
                 FROM installed_plugins WHERE name = ?1",
                [name],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, i64>(5)?,
                    ))
                },
            )
            .optional()?
            .with_context(|| format!("Plugin '{name}' is not installed"))?;
        Ok(PluginInfo {
            manifest: Manifest::from_toml(&values.0).context("Invalid manifest in SQLite store")?,
            source: values.1,
            revision: values.2,
            source_ref: values.3,
            checksum: values.4,
            installed_at: values.5,
        })
    }

    pub fn verify(&self, name: Option<&str>) -> Result<Vec<String>> {
        let plugins = if let Some(name) = name {
            vec![self.info(name)?]
        } else {
            self.list_info()?
        };
        let mut verified = Vec::new();
        for info in plugins {
            let (root, manifest) = self.load(&info.manifest.name)?;
            let checksum = sha256_file(&manifest.entrypoint(&root)?)?;
            ensure!(
                !info.checksum.is_empty() && checksum == info.checksum,
                "Plugin '{}' checksum mismatch",
                manifest.name
            );
            verified.push(manifest.name);
        }
        Ok(verified)
    }

    pub fn update(&self, name: &str) -> Result<Manifest> {
        let info = self.info(name)?;
        let source = info
            .source
            .as_deref()
            .with_context(|| format!("Plugin '{name}' has no recorded source"))?;
        if Path::new(source).is_dir() {
            return self.install_directory(
                Path::new(source),
                Some(source.to_owned()),
                None,
                None,
                InstallMode::Update,
            );
        }
        ensure!(
            source.starts_with("https://"),
            "Plugin source is not updateable"
        );
        let (checkout, destination, resolved_revision) =
            checkout_git(source, info.source_ref.as_deref())?;
        let manifest = Manifest::read(&destination)?;
        ensure!(
            manifest.name == name,
            "Updated plugin name does not match '{name}'"
        );
        let result = self.install_directory(
            &destination,
            Some(source.to_owned()),
            Some(resolved_revision),
            info.source_ref,
            InstallMode::Update,
        );
        drop(checkout);
        result
    }

    pub fn update_all(&self) -> Vec<(String, Result<Manifest>)> {
        match self.list_info() {
            Ok(plugins) => plugins
                .into_iter()
                .map(|plugin| {
                    let name = plugin.manifest.name;
                    eprintln!("Updating {name}");
                    let result = self.update(&name);
                    (name, result)
                })
                .collect(),
            Err(error) => vec![("*".to_owned(), Err(error))],
        }
    }

    pub fn outdated(&self) -> Result<Vec<UpdateStatus>> {
        let infos = self.list_info()?;
        let mut results = Vec::with_capacity(infos.len());
        std::thread::scope(|scope| -> Result<()> {
            let handles: Vec<_> = infos
                .into_iter()
                .map(|info| scope.spawn(move || self.outdated_one(info)))
                .collect();
            for handle in handles {
                results.push(
                    handle
                        .join()
                        .map_err(|_| anyhow::anyhow!("outdated worker panicked"))??,
                );
            }
            Ok(())
        })?;
        Ok(results)
    }

    fn outdated_one(&self, info: PluginInfo) -> Result<UpdateStatus> {
        let installed = info.manifest.version.clone();
        let Some(source) = info.source.as_deref() else {
            return Ok(UpdateStatus {
                name: info.manifest.name,
                installed_version: installed,
                available_version: None,
                update_available: false,
            });
        };
        let available = if Path::new(source).is_dir() {
            Manifest::read(Path::new(source))?.version
        } else {
            let (_checkout, root, _revision) = checkout_git(source, info.source_ref.as_deref())?;
            Manifest::read(&root)?.version
        };
        let update_available = versions_differ(&installed, &available);
        Ok(UpdateStatus {
            name: info.manifest.name,
            installed_version: installed,
            available_version: Some(available),
            update_available,
        })
    }

    pub fn doctor(&self, repair: bool) -> Result<DoctorReport> {
        fs::create_dir_all(self.plugins())?;
        let connection = self.connect()?;
        let database_names = {
            let mut statement = connection.prepare("SELECT name FROM installed_plugins")?;
            statement
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<rusqlite::Result<BTreeSet<_>>>()?
        };
        let mut disk_names = BTreeSet::new();
        let mut issues = Vec::new();
        let mut repairs = Vec::new();
        for entry in fs::read_dir(self.plugins())? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with(".install-")
                || name.starts_with(".remove-")
                || name.starts_with(".rollback-")
            {
                issues.push(format!("stale transaction directory: {name}"));
                if repair {
                    let candidate = if name.starts_with(".remove-") {
                        entry.path().join("package")
                    } else {
                        entry.path().join("previous")
                    };
                    if candidate.is_dir() {
                        if let Ok(manifest) = Manifest::read(&candidate) {
                            if database_names.contains(&manifest.name) {
                                let stored = self.info(&manifest.name)?;
                                if stored.manifest == manifest {
                                    let destination = self.plugins().join(&manifest.name);
                                    let destination_matches = Manifest::read(&destination)
                                        .is_ok_and(|current| current == stored.manifest);
                                    if !destination_matches {
                                        if let Ok(metadata) = fs::symlink_metadata(&destination) {
                                            ensure!(
                                                metadata.is_dir(),
                                                "Refusing to replace non-directory {}",
                                                destination.display()
                                            );
                                            fs::remove_dir_all(&destination)?;
                                        }
                                        fs::rename(&candidate, &destination)?;
                                        disk_names.insert(manifest.name.clone());
                                        repairs.push(format!(
                                            "restored interrupted transaction for {}",
                                            manifest.name
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    fs::remove_dir_all(entry.path())?;
                    repairs.push(format!("removed {name}"));
                }
                continue;
            }
            if entry.file_type()?.is_dir() {
                disk_names.insert(name);
            }
        }
        for name in database_names.difference(&disk_names) {
            issues.push(format!("database entry without plugin directory: {name}"));
            if repair {
                connection.execute("DELETE FROM installed_plugins WHERE name = ?1", [name])?;
                repairs.push(format!("removed stale database entry {name}"));
            }
        }
        for name in disk_names.difference(&database_names) {
            issues.push(format!("plugin directory without database entry: {name}"));
            if repair {
                let root = self.plugins().join(name);
                let manifest = Manifest::read(&root)?;
                ensure!(
                    manifest.name == *name,
                    "Orphan plugin directory name mismatch: {name}"
                );
                let checksum = sha256_file(&manifest.entrypoint(&root)?)?;
                connection.execute(
                    "INSERT INTO installed_plugins (name, manifest, checksum)
                     VALUES (?1, ?2, ?3)",
                    params![name, toml::to_string(&manifest)?, checksum],
                )?;
                repairs.push(format!("recovered plugin metadata for {name}"));
            }
        }
        for name in database_names.intersection(&disk_names) {
            let info = self.info(name)?;
            match self.load(name) {
                Ok((root, manifest)) => {
                    let checksum = sha256_file(&manifest.entrypoint(&root)?)?;
                    if info.checksum != checksum {
                        issues.push(format!("checksum mismatch or missing checksum: {name}"));
                        if repair && info.checksum.is_empty() {
                            connection.execute(
                                "UPDATE installed_plugins SET checksum = ?2 WHERE name = ?1",
                                params![name, checksum],
                            )?;
                            repairs.push(format!("recorded checksum for {name}"));
                        }
                    }
                }
                Err(error) => issues.push(format!("invalid plugin {name}: {error:#}")),
            }
        }
        let installed_names = {
            let mut statement = connection.prepare("SELECT name FROM installed_plugins")?;
            statement
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<rusqlite::Result<BTreeSet<_>>>()?
        };
        for kind in ["config", "data", "cache"] {
            let parent = self.home.join(kind);
            let entries = match fs::read_dir(&parent) {
                Ok(entries) => entries,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => {
                    return Err(error).with_context(|| format!("Read {}", parent.display()));
                }
            };
            for entry in entries {
                let entry = entry?;
                if !entry.file_type()?.is_dir() {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().into_owned();
                if installed_names.contains(&name) {
                    continue;
                }
                issues.push(format!("orphaned {kind} directory: {name}"));
                if repair {
                    fs::remove_dir_all(entry.path())?;
                    repairs.push(format!("removed orphaned {kind}/{name}"));
                }
            }
        }
        Ok(DoctorReport { issues, repairs })
    }

    #[doc(hidden)]
    pub fn load(&self, name: &str) -> Result<(PathBuf, Manifest)> {
        validate_name(name)?;
        let stored = self
            .connect()?
            .query_row(
                "SELECT manifest FROM installed_plugins WHERE name = ?1",
                [name],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .with_context(|| {
                format!("Plugin '{name}' is not installed; use dm install <source>")
            })?;
        let stored = Manifest::from_toml(&stored).context("Invalid manifest in SQLite store")?;
        let root = self.plugins().join(name);
        let metadata = fs::symlink_metadata(&root)
            .with_context(|| format!("Installed plugin '{name}' is missing from disk"))?;
        ensure!(
            metadata.is_dir(),
            "Installed plugin must be a regular directory"
        );
        let root = fs::canonicalize(root)?;
        let manifest = Manifest::read(&root)?;
        ensure!(
            manifest == stored,
            "Installed plugin manifest differs from SQLite metadata"
        );
        Ok((root, manifest))
    }

    pub fn uninstall(&self, name: &str) -> Result<()> {
        validate_name(name)?;
        let connection = self.connect()?;
        ensure!(
            connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM installed_plugins WHERE name = ?1)",
                [name],
                |row| row.get::<_, bool>(0),
            )?,
            "Plugin '{name}' is not installed"
        );
        let path = self.plugins().join(name);
        let metadata = fs::symlink_metadata(&path)
            .with_context(|| format!("Plugin '{name}' is not installed"))?;
        ensure!(
            metadata.is_dir(),
            "Installed plugin must be a regular directory"
        );
        let manifest = Manifest::read(&path).ok();
        if let Some(manifest) = manifest.as_ref() {
            if let Some(hook) = manifest.hooks.pre_uninstall.as_deref() {
                self.run_hook(&path, hook, "pre-uninstall", manifest)?;
            }
        }
        // Rename first so a database error can restore the complete installation.
        let stage = tempfile::Builder::new()
            .prefix(".remove-")
            .tempdir_in(self.plugins())?;
        let removed = stage.path().join("package");
        fs::rename(&path, &removed).context("Stage plugin removal")?;
        if let Some(manifest) = manifest.as_ref() {
            if let Some(hook) = manifest.hooks.post_uninstall.as_deref() {
                if let Err(error) = self.run_hook(&removed, hook, "post-uninstall", manifest) {
                    let _ = fs::rename(&removed, &path);
                    return Err(error).context("Run post-uninstall hook");
                }
            }
        }
        if let Err(error) =
            connection.execute("DELETE FROM installed_plugins WHERE name = ?1", [name])
        {
            let _ = fs::rename(&removed, &path);
            return Err(error).context("Remove plugin metadata");
        }
        for directory in self.per_plugin_directories(name) {
            let _ = fs::remove_dir_all(directory);
        }
        let _ = fs::remove_dir_all(self.backups().join(name));
        Ok(())
    }

    fn copy_hooks(&self, source: &Path, package: &Path, manifest: &Manifest) -> Result<()> {
        for hook in [
            manifest.hooks.post_install.as_deref(),
            manifest.hooks.pre_uninstall.as_deref(),
            manifest.hooks.post_uninstall.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            let from = source.join(hook);
            let to = package.join(hook);
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&from, &to).with_context(|| format!("Copy hook '{hook}'"))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = fs::metadata(&from)?.permissions().mode();
                fs::set_permissions(&to, fs::Permissions::from_mode(mode | 0o111))?;
            }
        }
        Ok(())
    }

    fn run_hook(&self, root: &Path, hook: &str, phase: &str, manifest: &Manifest) -> Result<()> {
        let root = fs::canonicalize(root).context("Resolve plugin hook directory")?;
        let executable = root.join(hook);
        let metadata =
            fs::symlink_metadata(&executable).with_context(|| format!("Missing hook '{hook}'"))?;
        ensure!(metadata.is_file(), "Hook '{hook}' must be a regular file");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            ensure!(
                metadata.permissions().mode() & 0o111 != 0,
                "Hook '{hook}' is not executable"
            );
        }
        let mut command = Command::new(&executable);
        command.env_clear();
        inherit_safe_environment(&mut command, manifest);
        command
            .arg(phase)
            .current_dir(&root)
            .env("DM_PLUGIN_HOME", fs::canonicalize(&self.home)?)
            .env("DM_PLUGIN_DIR", &root)
            .env("DM_HOOK_PHASE", phase);
        let status = command
            .status()
            .with_context(|| format!("Run {phase} hook '{hook}'"))?;
        ensure!(status.success(), "Plugin hook '{hook}' failed for {phase}");
        Ok(())
    }

    pub fn run(&self, name: &str, args: &[OsString]) -> Result<i32> {
        let (root, manifest) = self.load(name)?;
        let config_dir = self.home.join("config").join(name);
        let data_dir = self.home.join("data").join(name);
        let cache_dir = self.home.join("cache").join(name);
        for directory in [&config_dir, &data_dir, &cache_dir] {
            fs::create_dir_all(directory)?;
        }
        let mut command = Command::new(manifest.entrypoint(&root)?);
        command.env_clear();
        inherit_safe_environment(&mut command, &manifest);
        let status = command
            .args(args)
            .env("DM_PLUGIN_API_VERSION", API_VERSION.to_string())
            .env("DM_PLUGIN_CAPABILITIES", "config-dirs-v1")
            .env("DM_PLUGIN_DIR", &root)
            .env("DM_PLUGIN_HOME", fs::canonicalize(&self.home)?)
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
        Ok(code)
    }
}

#[doc(hidden)]
pub fn progress_bar_for(len: u64, terminal: bool) -> ProgressBar {
    if terminal {
        ProgressBar::new(len)
    } else {
        ProgressBar::hidden()
    }
}

fn progress_bar(len: u64) -> ProgressBar {
    use std::io::IsTerminal;
    progress_bar_for(len, std::io::stderr().is_terminal())
}

fn checkout_git(
    source: &str,
    revision: Option<&str>,
) -> Result<(tempfile::TempDir, PathBuf, String)> {
    ensure!(
        source.starts_with("https://") && source.len() > 8,
        "Source must be an HTTPS Git repository URL"
    );
    if let Some(revision) = revision {
        ensure!(
            !revision.is_empty()
                && !revision.starts_with('-')
                && !revision.chars().any(char::is_whitespace),
            "Git revision must be nonempty and contain no whitespace"
        );
    }
    let checkout = tempfile::tempdir()?;
    let destination = checkout.path().join("source");
    let bar = progress_bar(2);
    bar.set_message("Cloning plugin repository");
    eprintln!("Cloning {source}");
    let mut clone = Command::new("git");
    clone.args([
        "-c",
        "core.hooksPath=/dev/null",
        "-c",
        "protocol.allow=never",
        "-c",
        "protocol.https.allow=always",
        "clone",
    ]);
    if revision.is_none() {
        clone.args(["--depth", "1"]);
    }
    let output = clone
        .arg("--")
        .arg(source)
        .arg(&destination)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .context("Fetch plugin; HTTPS installation requires Git")?;
    ensure!(
        output.status.success(),
        "Git could not fetch the plugin\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    bar.inc(1);
    if let Some(revision) = revision {
        let output = Command::new("git")
            .args(["-c", "core.hooksPath=/dev/null", "checkout", "--detach"])
            .arg(revision)
            .current_dir(&destination)
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()?;
        ensure!(
            output.status.success(),
            "Git revision '{revision}' could not be checked out\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&destination)
        .output()?;
    ensure!(
        output.status.success(),
        "Could not resolve plugin Git revision"
    );
    bar.inc(1);
    bar.finish_and_clear();
    let resolved = String::from_utf8(output.stdout)?.trim().to_owned();
    Ok((checkout, destination, resolved))
}

#[doc(hidden)]
pub fn github_repository(source: &str) -> Option<(&str, &str)> {
    let rest = source.strip_prefix("https://github.com/")?;
    let rest = rest.trim_end_matches('/');
    let rest = rest.strip_suffix(".git").unwrap_or(rest);
    let (owner, repository) = rest.split_once('/')?;
    if repository.contains('/') {
        return None;
    }
    let valid = |value: &str| {
        !value.is_empty()
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    };
    (valid(owner) && valid(repository)).then_some((owner, repository))
}

pub fn prebuilt_target_label() -> Option<&'static str> {
    prebuilt_target_label_for(env!("DM_HOST_TARGET"))
}

#[doc(hidden)]
pub fn prebuilt_target_label_for(target: &str) -> Option<&'static str> {
    match target {
        "aarch64-apple-darwin" => Some("aarch64-macos"),
        "x86_64-apple-darwin" => Some("x86_64-macos"),
        "x86_64-unknown-linux-gnu" | "x86_64-unknown-linux-musl" => Some("x86_64-linux"),
        "aarch64-unknown-linux-gnu" | "aarch64-unknown-linux-musl" => Some("aarch64-linux"),
        "x86_64-pc-windows-msvc" => Some("x86_64-windows"),
        _ => None,
    }
}

#[doc(hidden)]
pub fn release_tag_candidates(manifest: &Manifest, revision: Option<&str>) -> Vec<String> {
    let mut candidates = Vec::new();
    if let Some(revision) = revision {
        if revision.starts_with('v') {
            candidates.push(revision.to_owned());
        } else if semver::Version::parse(revision).is_ok() {
            candidates.push(format!("v{revision}"));
        }
    }
    candidates.push(format!("v{}", manifest.version));
    candidates.dedup();
    candidates
}

fn download_prebuilt_asset(url: &str, destination: &Path) -> bool {
    Command::new("curl")
        .args([
            "-fsSL",
            "--retry",
            "3",
            "--connect-timeout",
            "15",
            "--output",
        ])
        .arg(destination)
        .arg(url)
        .output()
        .is_ok_and(|output| output.status.success())
}

fn verify_optional_prebuilt_checksum(binary_url: &str, binary: &Path) -> Result<()> {
    let checksum_path = binary.with_extension("sha256");
    let downloaded = download_prebuilt_asset(&format!("{binary_url}.sha256"), &checksum_path);
    if !downloaded {
        eprintln!("warning: prebuilt plugin has no SHA-256 sidecar; trusting HTTPS transport");
        return Ok(());
    }
    let expected = fs::read_to_string(&checksum_path)?;
    let _ = fs::remove_file(&checksum_path);
    let expected = expected
        .split_whitespace()
        .next()
        .context("Empty prebuilt SHA-256 sidecar")?;
    ensure!(
        expected.len() == 64 && expected.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "Invalid prebuilt SHA-256 sidecar"
    );
    let actual = sha256_file(binary)?;
    ensure!(
        actual.eq_ignore_ascii_case(expected),
        "Prebuilt plugin SHA-256 mismatch"
    );
    Ok(())
}

fn try_download_prebuilt(
    source: Option<&str>,
    manifest: &Manifest,
    revision: Option<&str>,
    destination: &Path,
) -> Result<()> {
    let source = source.context("Plugin source is not a GitHub repository")?;
    let (owner, repository) =
        github_repository(source).context("Prebuilt plugins require a GitHub HTTPS source")?;
    let target =
        prebuilt_target_label().context("No prebuilt plugin is published for this host target")?;
    let asset = format!(
        "{}-{}{}",
        manifest.binary_name(),
        target,
        std::env::consts::EXE_SUFFIX
    );
    let bar = progress_bar(1);
    bar.set_message("Downloading prebuilt plugin");
    for tag in release_tag_candidates(manifest, revision) {
        let url =
            format!("https://github.com/{owner}/{repository}/releases/download/{tag}/{asset}");
        if download_prebuilt_asset(&url, destination) {
            verify_optional_prebuilt_checksum(&url, destination)?;
            bar.inc(1);
            bar.finish_and_clear();
            return Ok(());
        }
    }
    bar.finish_and_clear();
    bail!(
        "No prebuilt plugin '{}' found for {target}; source builds are disabled",
        asset
    )
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[doc(hidden)]
pub fn versions_differ(installed: &str, available: &str) -> bool {
    match (
        semver::Version::parse(installed),
        semver::Version::parse(available),
    ) {
        (Ok(installed), Ok(available)) => available > installed,
        _ => installed != available,
    }
}

fn inherit_safe_environment(command: &mut Command, manifest: &Manifest) {
    const SAFE: &[&str] = &[
        "COLORTERM",
        "COMSPEC",
        "HOME",
        "LANG",
        "NO_COLOR",
        "PATH",
        "PATHEXT",
        "SYSTEMROOT",
        "TEMP",
        "TERM",
        "TMP",
        "TMPDIR",
        "TZ",
        "USERPROFILE",
        "WINDIR",
    ];
    for (name, value) in env::vars_os() {
        let text = name.to_string_lossy();
        if SAFE.contains(&text.as_ref())
            || text.starts_with("LC_")
            || manifest.environment.iter().any(|allowed| allowed == &text)
        {
            command.env(name, value);
        }
    }
}
