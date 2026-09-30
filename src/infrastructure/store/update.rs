use anyhow::{Context, Result, ensure};
use log::info;
use std::path::Path;

use crate::Manifest;

use super::{InstallMode, PluginInfo, PluginStore, UpdateStatus, checkout_git, versions_differ};

impl PluginStore {
    pub fn update(&self, name: &str) -> Result<Manifest> {
        info!("updating plugin {name}");
        let info = self.info(name)?;
        let source = info
            .source
            .as_deref()
            .with_context(|| format!("Plugin '{name}' has no recorded source"))?;
        if let Some(repository) = super::release::repository(source) {
            let tag = super::release::latest_tag(repository)?;
            let target = info.source_ref.as_deref().unwrap_or(env!("DM_HOST_TARGET"));
            let (_temporary, package) = super::release::package(repository, name, target, &tag)?;
            return self.install_directory(
                &package,
                Some(source.into()),
                Some(tag),
                Some(target.into()),
                InstallMode::Update,
            );
        }
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
            checkout_git(source, info.source_ref.as_deref(), self.progress_enabled())?;
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
                    info!("updating plugin {name}");
                    let result = self.update(&name);
                    (name, result)
                })
                .collect(),
            Err(error) => vec![("*".to_owned(), Err(error))],
        }
    }

    pub fn outdated(&self) -> Result<Vec<UpdateStatus>> {
        let infos = self.list_info()?;
        dm_plugin_support::parallel::map(infos, self.update_check_concurrency, |info| {
            self.outdated_one(info)
        })?
        .into_iter()
        .collect()
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
        let available = if let Some(repository) = super::release::repository(source) {
            let tag = super::release::latest_tag(repository)?;
            let target = info.source_ref.as_deref().unwrap_or(env!("DM_HOST_TARGET"));
            let (_temporary, package) =
                super::release::package(repository, &info.manifest.name, target, &tag)?;
            Some(Manifest::read(&package)?.version)
        } else if source.starts_with("https://") {
            let (_checkout, root, _revision) =
                checkout_git(source, info.source_ref.as_deref(), self.progress_enabled())?;
            Some(Manifest::read(&root)?.version)
        } else if Path::new(source).is_dir() {
            Some(Manifest::read(Path::new(source))?.version)
        } else {
            // The recorded local source is gone: installers unpack plugins into
            // a temporary directory, so a missing directory only means this
            // plugin cannot be compared, not that the whole check failed.
            None
        };
        let update_available = available
            .as_deref()
            .is_some_and(|available| versions_differ(&installed, available));
        Ok(UpdateStatus {
            name: info.manifest.name,
            installed_version: installed,
            available_version: available,
            update_available,
        })
    }
}
