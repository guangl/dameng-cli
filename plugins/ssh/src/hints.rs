use anyhow::Error;

/// Return a short, actionable hint for an SSH plugin error.
#[doc(hidden)]
pub fn ssh_hint(error: &Error) -> String {
    let text = error
        .chain()
        .map(|cause| cause.to_string())
        .collect::<Vec<_>>()
        .join("\n")
        .to_lowercase();

    if text.contains("not configured") {
        return "请先运行 `dm ssh add <name>` 配置服务器，或用 `dm ssh list` 查看已保存的连接。"
            .into();
    }
    if text.contains("sshpass") {
        return "密码认证的测试/登录需要安装 `sshpass`；也可改用密钥认证。".into();
    }
    if text.contains("password") || text.contains("key") {
        return "请通过 `--password` 或 `--key` 提供认证，或在终端下运行以交互输入。".into();
    }
    if text.contains("sqlite") || text.contains("table") || text.contains("store") {
        return "SSH 数据存储异常，请检查插件数据目录中的 servers.sqlite3。".into();
    }
    if text.contains("required") {
        return "缺少必填项；在终端下运行可交互输入，或显式传入对应参数。".into();
    }

    "使用 `dm ssh --help` 查看可用子命令和参数。".into()
}
