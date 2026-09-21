use crate::{API_VERSION, Manifest, manifest::validate_name};
use anyhow::{Context, Result, bail, ensure};
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
    pub enabled: bool,
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
        let home = if let Some(home) = env::var_os("DM_HOME") {
            ensure!(!home.is_empty(), "DM_HOME must not be empty");
            PathBuf::from(home)
        } else if cfg!(windows) {
            PathBuf::from(env::var_os("LOCALAPPDATA").context("Set DM_HOME or LOCALAPPDATA")?)
                .join("dm")
        } else if let Some(data) = env::var_os("XDG_DATA_HOME").filter(|s| !s.is_empty()) {
            let data = PathBuf::from(data);
            ensure!(data.is_absolute(), "XDG_DATA_HOME must be absolute");
            data.join("dm")
        } else {
            PathBuf::from(env::var_os("HOME").context("Set DM_HOME or HOME")?)
                .join(".local/share/dm")
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

    fn connect(&self) -> Result<Connection> {
        fs::create_dir_all(&self.home).context("Create dm data directory")?;
        let connection = Connection::open(self.database()).context("Open SQLite plugin store")?;
        connection
            .busy_timeout(std::time::Duration::from_secs(5))
            .context("Configure SQLite plugin store")?;
        let schema_version: u32 =
            connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        ensure!(
            schema_version <= 2,
            "SQLite plugin store schema {schema_version} is newer than this dm supports"
        );
        connection
            .execute_batch(
                "PRAGMA journal_mode = WAL;
                 PRAGMA foreign_keys = ON;
                 CREATE TABLE IF NOT EXISTS installed_plugins (
                     name TEXT PRIMARY KEY,
                     manifest TEXT NOT NULL,
                     installed_at INTEGER NOT NULL DEFAULT (unixepoch()),
                     source TEXT,
                     revision TEXT,
                     source_ref TEXT,
                     checksum TEXT NOT NULL DEFAULT '',
                     enabled INTEGER NOT NULL DEFAULT 1
                 ) STRICT;
                 CREATE TABLE IF NOT EXISTS registry (
                     name TEXT PRIMARY KEY,
                     source TEXT NOT NULL,
                     updated_at INTEGER NOT NULL DEFAULT (unixepoch())
                 ) STRICT;",
            )
            .context("Initialize SQLite plugin store")?;
        if schema_version == 1 {
            connection.execute_batch(
                "BEGIN IMMEDIATE;
                 ALTER TABLE installed_plugins ADD COLUMN source TEXT;
                 ALTER TABLE installed_plugins ADD COLUMN revision TEXT;
                 ALTER TABLE installed_plugins ADD COLUMN source_ref TEXT;
                 ALTER TABLE installed_plugins ADD COLUMN checksum TEXT NOT NULL DEFAULT '';
                 ALTER TABLE installed_plugins ADD COLUMN enabled INTEGER NOT NULL DEFAULT 1;
                 PRAGMA user_version = 2;
                 COMMIT;",
            )?;
        } else if schema_version == 0 {
            connection.execute_batch("PRAGMA user_version = 2;")?;
        }
        Ok(connection)
    }

    fn registered_source(&self, name: &str) -> Result<String> {
        self.connect()?
            .query_row(
                "SELECT source FROM registry WHERE name = ?1",
                [name],
                |row| row.get(0),
            )
            .optional()?
            .with_context(|| format!("Plugin '{name}' is not registered"))
    }

    pub fn install(&self, source: &str) -> Result<Manifest> {
        self.install_with_revision(source, None)
    }

    pub fn install_with_revision(&self, source: &str, revision: Option<&str>) -> Result<Manifest> {
        self.install_with_consent(source, revision, false)
    }

    pub fn install_with_consent(
        &self,
        source: &str,
        revision: Option<&str>,
        accept_permissions: bool,
    ) -> Result<Manifest> {
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
                accept_permissions,
            );
        }
        let requested_name = (!source.starts_with("https://")).then_some(source);
        let resolved;
        let source = if source.starts_with("https://") {
            source
        } else {
            validate_name(source)?;
            resolved = self.registered_source(source).with_context(|| {
                format!(
                    "Unknown plugin '{source}'; use a local directory, HTTPS Git URL, or dm registry add"
                )
            })?;
            &resolved
        };
        ensure!(
            source.starts_with("https://") && source.len() > 8,
            "Source must be a local plugin directory or HTTPS Git repository URL"
        );
        let (checkout, destination, resolved_revision) = checkout_git(source, revision)?;
        if let Some(name) = requested_name {
            ensure!(
                Manifest::read(&destination)?.name == name,
                "Registry name '{name}' does not match the fetched plugin manifest"
            );
        }
        let result = self.install_directory(
            &destination,
            Some(source.to_owned()),
            Some(resolved_revision),
            revision.map(str::to_owned),
            InstallMode::New,
            accept_permissions,
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
        accept_permissions: bool,
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
        let previous_manifest = if installed {
            Some(self.info(&manifest.name)?.manifest)
        } else {
            None
        };
        ensure!(
            accept_permissions || !manifest.requests_consent_from(previous_manifest.as_ref()),
            "Plugin '{}' requests new permissions or environment variables; review its manifest and rerun with --accept-permissions",
            manifest.name
        );
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
        validate_crate(&source, &manifest)?;
        let build = stage.path().join("build");
        let status = Command::new("cargo")
            .args(["build", "--release", "--locked", "--manifest-path"])
            .arg(source.join("Cargo.toml"))
            .args([
                "--bin",
                &manifest.binary_name(),
                "--target",
                env!("DM_HOST_TARGET"),
                "--target-dir",
            ])
            .arg(&build)
            .current_dir(&source)
            .status()
            .context("Build Rust plugin; install the Rust toolchain and Cargo first")?;
        ensure!(
            status.success(),
            "Rust plugin build failed; no plugin was installed"
        );
        ensure!(
            Manifest::read(&source)? == manifest,
            "Plugin manifest changed during build"
        );
        fs::create_dir(&package)?;
        fs::copy(
            build
                .join(env!("DM_HOST_TARGET"))
                .join("release")
                .join(manifest.executable_name()),
            package.join(manifest.executable_name()),
        )
        .context("Copy compiled Rust plugin")?;
        fs::write(
            package.join(crate::MANIFEST_FILE),
            toml::to_string(&manifest)?,
        )?;
        manifest.entrypoint(&package)?;
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
        let transaction = connection.transaction()?;
        let database_result = match mode {
            InstallMode::New => transaction.execute(
                "INSERT INTO installed_plugins
                 (name, manifest, source, revision, source_ref, checksum, enabled)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1)",
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
                "SELECT manifest, source, revision, source_ref, checksum, installed_at, enabled
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
                        row.get::<_, bool>(6)?,
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
            enabled: values.6,
        })
    }

    pub fn set_enabled(&self, name: &str, enabled: bool) -> Result<()> {
        validate_name(name)?;
        ensure!(
            self.connect()?.execute(
                "UPDATE installed_plugins SET enabled = ?2 WHERE name = ?1",
                params![name, enabled],
            )? == 1,
            "Plugin '{name}' is not installed"
        );
        Ok(())
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
        self.update_with_consent(name, false)
    }

    pub fn update_with_consent(&self, name: &str, accept_permissions: bool) -> Result<Manifest> {
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
                accept_permissions,
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
            accept_permissions,
        );
        drop(checkout);
        result
    }

    pub fn update_all(&self) -> Vec<(String, Result<Manifest>)> {
        self.update_all_with_consent(false)
    }

    pub fn update_all_with_consent(
        &self,
        accept_permissions: bool,
    ) -> Vec<(String, Result<Manifest>)> {
        match self.list_info() {
            Ok(plugins) => plugins
                .into_iter()
                .map(|plugin| {
                    let name = plugin.manifest.name;
                    let result = self.update_with_consent(&name, accept_permissions);
                    (name, result)
                })
                .collect(),
            Err(error) => vec![("*".to_owned(), Err(error))],
        }
    }

    pub fn outdated(&self) -> Result<Vec<UpdateStatus>> {
        self.list_info()?
            .into_iter()
            .map(|info| {
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
                    let (_checkout, root, _revision) =
                        checkout_git(source, info.source_ref.as_deref())?;
                    Manifest::read(&root)?.version
                };
                let update_available = versions_differ(&installed, &available);
                Ok(UpdateStatus {
                    name: info.manifest.name,
                    installed_version: installed,
                    available_version: Some(available),
                    update_available,
                })
            })
            .collect()
    }

    pub fn search(&self, query: &str) -> Result<Vec<(String, String)>> {
        Ok(filter_registry(self.registry_list()?, query))
    }

    pub fn search_remote(&self, url: &str, query: &str) -> Result<Vec<(String, String)>> {
        Ok(filter_registry(
            crate::registry_index::fetch_registry_index(url)?,
            query,
        ))
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
            if name.starts_with(".install-") || name.starts_with(".remove-") {
                issues.push(format!("stale transaction directory: {name}"));
                if repair {
                    let candidate = if name.starts_with(".install-") {
                        entry.path().join("previous")
                    } else {
                        entry.path().join("package")
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

    fn load(&self, name: &str) -> Result<(PathBuf, Manifest)> {
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
        // Rename first so a database error can restore the complete installation.
        let stage = tempfile::Builder::new()
            .prefix(".remove-")
            .tempdir_in(self.plugins())?;
        let removed = stage.path().join("package");
        fs::rename(&path, &removed).context("Stage plugin removal")?;
        if let Err(error) =
            connection.execute("DELETE FROM installed_plugins WHERE name = ?1", [name])
        {
            let _ = fs::rename(&removed, &path);
            return Err(error).context("Remove plugin metadata");
        }
        for directory in self.per_plugin_directories(name) {
            let _ = fs::remove_dir_all(directory);
        }
        Ok(())
    }

    pub fn registry_add(&self, name: &str, source: &str) -> Result<()> {
        validate_name(name)?;
        ensure!(
            source.starts_with("https://") && source.len() > 8,
            "Registry source must be an HTTPS Git repository URL"
        );
        self.connect()?.execute(
            "INSERT INTO registry (name, source) VALUES (?1, ?2)
             ON CONFLICT(name) DO UPDATE SET source = excluded.source, updated_at = unixepoch()",
            params![name, source],
        )?;
        Ok(())
    }

    pub fn verify_registry_source(&self, source: &str) -> Result<()> {
        ensure!(
            source.starts_with("https://") && source.len() > 8,
            "Registry source must be an HTTPS Git repository URL"
        );
        let status = Command::new("git")
            .args([
                "-c",
                "protocol.https.allow=always",
                "-c",
                "protocol.allow=never",
                "ls-remote",
            ])
            .arg(source)
            .env("GIT_TERMINAL_PROMPT", "0")
            .status()
            .context("Verify registry source; HTTPS registry verification requires Git")?;
        ensure!(status.success(), "Git source is not reachable: {source}");
        Ok(())
    }

    pub fn registry_sync(&self, url: &str, prune: bool) -> Result<(usize, usize)> {
        let entries = crate::registry_index::fetch_registry_index(url)?;
        let mut connection = self.connect()?;
        let transaction = connection.transaction()?;
        for (name, source) in &entries {
            transaction.execute(
                "INSERT INTO registry (name, source) VALUES (?1, ?2)
                 ON CONFLICT(name) DO UPDATE SET source = excluded.source, updated_at = unixepoch()",
                params![name, source],
            )?;
        }
        let mut removed = 0;
        if prune {
            let existing = {
                let mut statement = transaction.prepare("SELECT name FROM registry")?;
                let names = statement.query_map([], |row| row.get::<_, String>(0))?;
                names.collect::<rusqlite::Result<Vec<_>>>()?
            };
            for name in existing {
                if !entries.iter().any(|(entry, _)| entry == &name) {
                    transaction.execute("DELETE FROM registry WHERE name = ?1", [&name])?;
                    removed += 1;
                }
            }
        }
        transaction.commit()?;
        Ok((entries.len(), removed))
    }

    pub fn registry_list(&self) -> Result<Vec<(String, String)>> {
        let connection = self.connect()?;
        let mut statement =
            connection.prepare("SELECT name, source FROM registry ORDER BY name")?;
        let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect::<rusqlite::Result<_>>().map_err(Into::into)
    }

    pub fn registry_remove(&self, name: &str) -> Result<()> {
        validate_name(name)?;
        ensure!(
            self.connect()?
                .execute("DELETE FROM registry WHERE name = ?1", [name])?
                == 1,
            "Plugin '{name}' is not registered"
        );
        Ok(())
    }

    pub fn run(&self, name: &str, args: &[OsString]) -> Result<i32> {
        ensure!(self.info(name)?.enabled, "Plugin '{name}' is disabled");
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
            .env("DM_HOME", fs::canonicalize(&self.home)?)
            .env("DM_PLUGIN_CONFIG_DIR", fs::canonicalize(config_dir)?)
            .env("DM_PLUGIN_DATA_DIR", fs::canonicalize(data_dir)?)
            .env("DM_PLUGIN_CACHE_DIR", fs::canonicalize(cache_dir)?)
            .status()
            .with_context(|| format!("Start plugin '{name}'"))?;
        if let Some(code) = status.code() {
            return Ok(code);
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            if let Some(signal) = status.signal() {
                return Ok(128 + signal);
            }
        }
        bail!("Plugin terminated without an exit code")
    }
}

fn validate_crate(source: &Path, manifest: &Manifest) -> Result<()> {
    let cargo: toml::Value = toml::from_str(
        &fs::read_to_string(source.join("Cargo.toml"))
            .context("Rust plugins must include Cargo.toml")?,
    )?;
    let package = cargo
        .get("package")
        .context("Plugin must be a Rust package")?;
    ensure!(
        package.get("version").and_then(toml::Value::as_str) == Some(&manifest.version),
        "Cargo package version must match dm-plugin.toml (use an explicit version)"
    );
    ensure!(
        cargo
            .get("dependencies")
            .and_then(|v| v.get("dm-plugin-sdk"))
            .is_some(),
        "Rust plugins must depend on dm-plugin-sdk"
    );
    ensure!(
        cargo
            .get("bin")
            .and_then(toml::Value::as_array)
            .is_some_and(|bins| bins
                .iter()
                .any(|bin| bin.get("name").and_then(toml::Value::as_str)
                    == Some(&manifest.binary_name()))),
        "Rust plugins must declare [[bin]] with name '{}'",
        manifest.binary_name()
    );
    Ok(())
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
    let status = clone
        .arg("--")
        .arg(source)
        .arg(&destination)
        .env("GIT_TERMINAL_PROMPT", "0")
        .status()
        .context("Fetch plugin; HTTPS installation requires Git")?;
    ensure!(status.success(), "Git could not fetch the plugin");
    if let Some(revision) = revision {
        let status = Command::new("git")
            .args(["-c", "core.hooksPath=/dev/null", "checkout", "--detach"])
            .arg(revision)
            .current_dir(&destination)
            .env("GIT_TERMINAL_PROMPT", "0")
            .status()?;
        ensure!(
            status.success(),
            "Git revision '{revision}' could not be checked out"
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
    let resolved = String::from_utf8(output.stdout)?.trim().to_owned();
    Ok((checkout, destination, resolved))
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

fn versions_differ(installed: &str, available: &str) -> bool {
    match (
        semver::Version::parse(installed),
        semver::Version::parse(available),
    ) {
        (Ok(installed), Ok(available)) => available > installed,
        _ => installed != available,
    }
}

fn filter_registry(entries: Vec<(String, String)>, query: &str) -> Vec<(String, String)> {
    let query = query.to_ascii_lowercase();
    entries
        .into_iter()
        .filter(|(name, source)| {
            name.to_ascii_lowercase().contains(&query)
                || source.to_ascii_lowercase().contains(&query)
        })
        .collect()
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
