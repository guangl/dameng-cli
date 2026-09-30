//! Plugin subcommands: install, list, info, uninstall and run.

use super::print_no_plugins;
use super::table;
use crate::{Config, PluginStore};
use anyhow::{Context, Result};
use std::ffi::OsString;

/// Install a prebuilt plugin from a directory or an HTTPS Git repository.
pub(super) fn install(
    store: &PluginStore,
    source: &str,
    rev: Option<&str>,
    replace: bool,
) -> Result<()> {
    let manifest = store.install_with_revision(source, rev, replace)?;
    println!("已安装 {} {}", manifest.name, manifest.version);
    Ok(())
}

/// Print installed plugins as a table, or as JSON when asked.
pub(super) fn list(store: &PluginStore, json: bool) -> Result<()> {
    let plugins = store.list_info()?;
    if json {
        println!("{}", serde_json::to_string_pretty(&plugins)?);
    } else if plugins.is_empty() {
        print_no_plugins();
    } else {
        println!("{}", table::render(&plugins));
    }
    Ok(())
}

/// Print installation metadata for one plugin.
pub(super) fn info(store: &PluginStore, name: &str, json: bool) -> Result<()> {
    let plugin = store.info(name)?;
    // Plugins configure themselves inside their own directory; expose the
    // paths so users can find (and edit) the right file.
    let directories = store.plugin_directories(&plugin.manifest.name);
    let config_file = Config::path_in(&directories[0]);
    if json {
        let mut value = serde_json::to_value(&plugin)?;
        value["paths"] = serde_json::json!({
            "config": directories[0],
            "data": directories[1],
            "cache": directories[2],
            "config_file": config_file,
            "config_file_present": config_file.is_file(),
        });
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        println!("名称： {}", plugin.manifest.name);
        println!("版本： {}", plugin.manifest.version);
        println!("来源： {}", plugin.source.as_deref().unwrap_or("unknown"));
        println!("修订： {}", plugin.revision.as_deref().unwrap_or("unknown"));
        println!("SHA-256: {}", plugin.checksum);
        if !plugin.manifest.environment.is_empty() {
            println!("继承环境变量： {}", plugin.manifest.environment.join(", "));
        }
        println!("配置目录： {}", directories[0].display());
        println!("数据目录： {}", directories[1].display());
        println!("缓存目录： {}", directories[2].display());
        println!(
            "配置文件： {} ({})",
            config_file.display(),
            if config_file.is_file() {
                "存在"
            } else {
                "不存在"
            }
        );
    }
    Ok(())
}

/// 卸载插件，默认保留配置和连接数据。
pub(super) fn uninstall(store: &PluginStore, name: &str, purge: bool, yes: bool) -> Result<()> {
    if purge {
        if !store.has_retained_data(name)? {
            store.info(name)?;
        }
        eprintln!("将清除插件 {name} 的配置、连接、缓存及备份：");
        for path in store.removal_paths(name) {
            eprintln!("  {}", path.display());
        }
        dm_plugin_support::interaction::confirm(
            dm_plugin_support::interaction::terminal_prompter(),
            yes,
            &format!("彻底卸载 {name}？"),
        )?;
    }
    store.uninstall_with_options(name, purge)?;
    println!(
        "已卸载 {name}。{}",
        if purge {
            "已清除数据。"
        } else {
            "配置和连接已保留，重新安装后可继续使用。"
        }
    );
    Ok(())
}

/// Run an installed plugin, forwarding every argument unchanged.
pub(super) fn run_plugin(store: &PluginStore, args: &[OsString]) -> Result<i32> {
    let name = args
        .first()
        .context("Missing plugin name")?
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Plugin name must be UTF-8"))?;
    let names = store.completion_names().unwrap_or_default();
    if !names.iter().any(|candidate| candidate == name) {
        let mut candidates = names;
        candidates.extend(
            [
                "list",
                "info",
                "install",
                "update",
                "uninstall",
                "doctor",
                "config",
                "completions",
                "self-update",
            ]
            .map(str::to_owned),
        );
        anyhow::bail!(
            "Plugin '{name}' is not installed；相近命令或插件：{}",
            dm_plugin_support::interaction::suggestions(name, &candidates)
        );
    }
    store.run(name, &args[1..])
}
