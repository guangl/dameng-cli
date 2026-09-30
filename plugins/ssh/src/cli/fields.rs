use clap::Args;
use std::path::PathBuf;

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
    #[arg(long, conflicts_with_all = ["key", "passphrase"])]
    pub(crate) password: Option<String>,
    /// 本机私钥路径。
    #[arg(long)]
    pub(crate) key: Option<PathBuf>,
    /// 私钥口令（编辑时保留；空字符串清除）。
    #[arg(long)]
    pub(crate) passphrase: Option<String>,
    /// 跳过保存摘要确认；适合脚本。
    #[arg(long)]
    pub(crate) yes: bool,
}
