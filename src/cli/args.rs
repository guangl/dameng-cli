//! Argument definitions for the `dm` command line.

use clap::{Parser, Subcommand};
use dm_plugin_support::config::ConfigCommand;
use std::ffi::OsString;

/// The `dm` command line.
#[derive(Parser)]
#[command(
    name = "dm",
    version,
    about = "安装、管理和运行达梦工具插件",
    after_help = "常用操作：\n  dm list                 查看已安装插件\n  dm update               检查插件更新\n  dm info <name>          查看插件配置与数据路径\n  dm <name> --help         查看插件命令\n  dm doctor               诊断安装问题"
)]
pub struct Cli {
    /// Subcommand to run.
    #[command(subcommand)]
    pub command: Command,
}

/// Every subcommand the host implements.
#[derive(Subcommand)]
pub enum Command {
    /// 从本地包目录或 HTTPS Git 仓库安装预编译插件。
    Install {
        source: String,
        /// 为 Release 安装包记录可持续更新的 GitHub owner/repository。
        #[arg(long, requires = "release_tag", conflicts_with = "rev")]
        release_source: Option<String>,
        #[arg(long, requires = "release_source")]
        release_tag: Option<String>,
        #[arg(long, requires = "release_source")]
        release_target: Option<String>,
        /// 固定 Git tag、分支或提交。
        #[arg(long)]
        rev: Option<String>,
        /// 覆盖同名插件，
        /// 保留配置、数据和缓存。
        #[arg(long)]
        replace: bool,
    },
    /// 查看已安装插件。
    List {
        #[arg(long)]
        json: bool,
    },
    /// 查看插件信息与配置路径。
    Info {
        name: String,
        #[arg(long)]
        json: bool,
    },
    /// 检查插件更新；指定名称或 --all 执行升级。
    Update {
        /// 升级该插件；省略时仅检查版本。
        name: Option<String>,
        /// 升级全部已安装插件。
        #[arg(long, conflicts_with = "name")]
        all: bool,
        /// 以 JSON 输出检查结果（不能与插件名或 --all 同用）。
        #[arg(long, conflicts_with_all = ["name", "all"])]
        json: bool,
    },
    /// 检查插件存储；--repair 修复可恢复问题。
    Doctor {
        /// 指定插件时检查该插件的使用环境。
        name: Option<String>,
        #[arg(long)]
        repair: bool,
        #[arg(long)]
        json: bool,
    },
    /// 从 GitHub Release 更新宿主程序。
    SelfUpdate {
        #[arg(long)]
        check: bool,
        #[arg(long)]
        version: Option<String>,
        /// 允许重装当前版本或降级。
        #[arg(long)]
        force: bool,
        /// 指定发布产物平台。
        #[arg(long)]
        target: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// 生成包含插件和连接名称的动态 shell 补全脚本。
    Completions { shell: super::Shell },
    /// 卸载插件，默认保留配置和连接数据。
    Uninstall {
        name: String,
        /// 同时清除配置、连接、缓存和备份；默认保留。
        #[arg(long)]
        purge: bool,
        /// 跳过清除数据的确认。
        #[arg(long)]
        yes: bool,
    },
    /// 创建或查看宿主配置。
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    #[command(hide = true)]
    Complete {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        words: Vec<String>,
    },
    /// 运行已安装插件，原样转发参数。
    #[command(external_subcommand)]
    Plugin(Vec<OsString>),
}
