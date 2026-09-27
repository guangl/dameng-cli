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
    if text.contains("already exist") {
        return "同名服务器已存在：加 `--replace` 覆盖其配置，或先 `dm ssh remove <name>` 再导入。"
            .into();
    }
    if text.contains("without overwriting") {
        return "导出文件已存在：换一个路径，或先删除该文件（导出从不覆盖已有文件）。".into();
    }
    if text.contains("export") || text.contains("import") || text.contains("passphrase") {
        return "导入/导出失败：请检查文件路径、导出版本与口令；加密导入导出需要在终端下输入并确认口令。"
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
