use super::fields::Fields;
use clap::{Parser, Subcommand};
use dm_plugin_support::config::ConfigCommand;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "dm db",
    about = "管理保存的数据库连接",
    after_help = "配置文件位于 config.toml；运行 dm info db 查看路径。\n\n常用操作：\n  dm db add prod       添加连接\n  dm db edit prod      修改连接，回车保留原值\n  dm db list           查看连接\n  dm db doctor         检查使用环境\n  dm db config init    创建配置示例\n  dm db export --file connections.json\n\n当前仅支持连接配置管理；test/exec 的数据库驱动尚未实现。"
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: DbCommand,
}

#[derive(Subcommand)]
pub(crate) enum DbCommand {
    /// 添加连接；同名连接需使用 --replace。
    Add {
        name: Option<String>,
        #[command(flatten)]
        fields: Fields,
        #[arg(long)]
        replace: bool,
    },
    /// 修改连接；省略的字段和密码会保留。
    Edit {
        name: String,
        #[command(flatten)]
        fields: Fields,
    },
    /// 查看保存的连接（不会显示密码）。
    List {
        #[arg(long)]
        json: bool,
    },
    /// 删除连接；终端下确认，脚本须使用 --yes。
    Remove {
        name: String,
        #[arg(long)]
        yes: bool,
    },
    /// 导出连接，默认不包含秘密；不覆盖已有文件。
    Export {
        #[arg(long)]
        file: Option<PathBuf>,
        #[arg(long)]
        include_passwords: bool,
    },
    /// 导入连接；--replace 覆盖同名配置。
    Import {
        file: PathBuf,
        #[arg(long)]
        replace: bool,
    },
    /// 检查本机工具、配置与连接文件。
    Doctor {
        #[arg(long)]
        json: bool,
    },
    /// 创建或查看本插件的配置。
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    /// 测试连接（数据库驱动尚未实现）。
    Test { name: Option<String> },
    /// 执行 SQL（数据库驱动尚未实现）。
    Exec {
        name: String,
        #[arg(conflicts_with = "file")]
        sql: Option<String>,
        #[arg(long)]
        file: Option<PathBuf>,
    },
}
