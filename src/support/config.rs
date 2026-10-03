//! Configuration discovery and safe initialization.
use anyhow::{Context, Result};
use clap::Subcommand;
use serde::Serialize;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

#[derive(Subcommand, Debug)]
pub enum ConfigCommand {
    /// 创建配置示例；已有文件不会被覆盖。
    Init,
    /// 查看有效配置及各项来源。
    Show {
        #[arg(long)]
        json: bool,
    },
    /// 显示配置文件路径。
    Path,
}
#[derive(Serialize)]
pub struct Setting {
    pub key: String,
    pub value: serde_json::Value,
    pub source: String,
}
pub fn setting(key: &str, value: impl Serialize, configured: bool) -> Setting {
    Setting {
        key: key.into(),
        value: serde_json::to_value(value).unwrap_or_default(),
        source: if configured { "config" } else { "default" }.into(),
    }
}
pub fn initialize(path: &Path, sample: &str) -> Result<()> {
    fs::create_dir_all(path.parent().context("配置目录不存在")?)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| format!("配置文件已存在或不可写：{}", path.display()))?;
    file.write_all(sample.as_bytes())?;
    println!("已创建配置：{}", path.display());
    Ok(())
}
pub fn show(path: &Path, settings: Vec<Setting>, json: bool) -> Result<()> {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({"path": path, "settings": settings}))?
        );
    } else {
        println!("配置文件：{}", path.display());
        for entry in settings {
            println!("{} = {} ({})", entry.key, entry.value, entry.source);
        }
    }
    Ok(())
}
