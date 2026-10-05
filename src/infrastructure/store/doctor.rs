use anyhow::{Context, Result, ensure};
use log::info;
use rusqlite::params;
use std::{collections::BTreeSet, fs};

use super::{DoctorReport, PluginStore, RESERVED_HOME_ENTRIES, sha256_file};
use crate::Manifest;
mod tables;
use tables::{foreign_store_tables, installed_plugin_names};
impl PluginStore {
    pub fn doctor(&self, repair: bool) -> Result<DoctorReport> {
        info!("running doctor repair={repair}");
        fs::create_dir_all(self.plugins())?;
        let connection = self.connect()?;
        let database_names = installed_plugin_names(&connection)?;
        let mut disk_names = BTreeSet::new();
        let mut issues = Vec::new();
        let mut repairs = Vec::new();
        let mut grouped_sources =
            self.reserved_sources_for_doctor(repair, &mut issues, &mut repairs)?;
        // The host store belongs to the host; a plugin that creates tables here
        // would tie its data to the host version and to other plugins.
        for table in foreign_store_tables(&connection)? {
            issues.push(format!(
                "unexpected table in the host store: {table}; plugins must create their own SQLite file below DM_PLUGIN_DATA_DIR instead"
            ));
        }
        for entry in fs::read_dir(self.plugins())? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if grouped_sources.contains(&entry.path()) && !database_names.contains(&name) {
                continue;
            }
            if name.starts_with(".install-")
                || name.starts_with(".remove-")
                || name.starts_with(".rollback-")
            {
                if self.protects_host_logs(&entry.path()) {
                    continue;
                }
                issues.push(format!("stale transaction directory: {name}"));
                if repair {
                    let candidate = if name.starts_with(".remove-") {
                        entry.path().join("package")
                    } else {
                        entry.path().join("previous")
                    };
                    if candidate.is_dir()
                        && let Ok(manifest) = Manifest::read(&candidate)
                        && database_names.contains(&manifest.name)
                    {
                        let stored = self.info(&manifest.name)?;
                        if stored.manifest == manifest {
                            let destination = self.plugins().join(&manifest.name);
                            if self.protects_host_logs(&destination) {
                                disk_names.insert(manifest.name.clone());
                                continue;
                            }
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
                    fs::remove_dir_all(entry.path())?;
                    repairs.push(format!("removed {name}"));
                }
                continue;
            }
            if entry.file_type()?.is_dir()
                && (!self.protects_host_logs(&entry.path())
                    || database_names.contains(&name)
                    || Manifest::read(&entry.path()).is_ok())
            {
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
        // Metadata recovery may reveal reserved plugins omitted from the first scan.
        if repair {
            grouped_sources.extend(self.reserved_sources_for_doctor(
                true,
                &mut issues,
                &mut repairs,
            )?);
        }
        let installed_names = {
            let mut statement = connection.prepare(
                "SELECT name FROM installed_plugins UNION SELECT name FROM retained_plugin_data",
            )?;
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
                if installed_names.contains(&name)
                    || self.protects_host_logs(&entry.path())
                    || grouped_sources.contains(&entry.path())
                {
                    continue;
                }
                issues.push(format!("orphaned {kind} directory: {name}"));
                if repair {
                    fs::remove_dir_all(entry.path())?;
                    repairs.push(format!("removed orphaned {kind}/{name}"));
                }
            }
        }
        for entry in fs::read_dir(&self.home)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if RESERVED_HOME_ENTRIES.contains(&name.as_str())
                || installed_names.contains(&name)
                || self.protects_host_logs(&entry.path())
            {
                continue;
            }
            issues.push(format!("orphaned plugin directory: {name}"));
            if repair {
                fs::remove_dir_all(entry.path())?;
                repairs.push(format!("removed orphaned plugin directory {name}"));
            }
        }
        Ok(DoctorReport { issues, repairs })
    }
}
