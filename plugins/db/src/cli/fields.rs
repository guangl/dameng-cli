use clap::Args;

#[derive(Args)]
pub(crate) struct Fields {
    /// 服务器地址。
    #[arg(long)]
    pub(crate) host: Option<String>,
    /// 端口。
    #[arg(long)]
    pub(crate) port: Option<u16>,
    /// 用户名。
    #[arg(long)]
    pub(crate) username: Option<String>,
    /// 密码（省略时交互输入；编辑时保留）。
    #[arg(long)]
    pub(crate) password: Option<String>,
    /// 数据库 schema。
    #[arg(long)]
    pub(crate) schema: Option<String>,
    /// 数据库驱动名称。
    #[arg(long)]
    pub(crate) driver: Option<String>,
    /// 跳过保存摘要确认；适合脚本。
    #[arg(long)]
    pub(crate) yes: bool,
    /// 清除已保存的 schema（仅用于 edit）。
    #[arg(long, conflicts_with = "schema")]
    pub(crate) clear_schema: bool,
}
