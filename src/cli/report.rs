//! Human-friendly error reporting for the `dm` CLI.
//!
//! Errors bubble up as `anyhow::Error` chains. This module keeps the full
//! diagnostic chain available (both on stderr and through the logging backend)
//! while adding a one-line summary and an actionable hint for every failure.

use anyhow::Error;

/// Print one error to stderr using a stable, greppable layout.
///
/// The full `anyhow` chain is preserved verbatim so scripts and tests that
/// scan stderr keep working, while humans get a short summary and a hint.
pub fn report(error: &Error) {
    log::error!("{error:#}");

    let summary = error
        .chain()
        .next()
        .map(ToString::to_string)
        .unwrap_or_else(|| error.to_string());
    let hint = hint_for(error);

    eprintln!("错误：{summary}");
    eprintln!("详情：{error:#}");
    eprintln!("提示：{hint}");
}

/// Choose an actionable hint from the full error chain.
///
/// Public so the hint table can be exercised from `tests/unit/report.rs`.
pub fn hint_for(error: &Error) -> String {
    let text = error
        .chain()
        .map(|cause| cause.to_string())
        .collect::<Vec<_>>()
        .join("\n")
        .to_lowercase();

    if text.contains("not installed") || text.contains("no recorded source") {
        return "请先运行 `dm install <source>` 安装插件，或用 `dm list` 查看已安装插件。".into();
    }
    if text.contains("already installed") {
        return "该插件已安装；升级请运行 `dm update <name>`，从新的包目录替换（保留配置与数据）请加 `--replace`。"
            .into();
    }
    if text.contains("not updateable") {
        return "该插件的来源不是可更新的仓库（例如安装脚本使用的临时目录）：请重新运行安装脚本，或运行 `dm install <新的包目录> --replace` 覆盖安装并保留配置与数据。"
            .into();
    }
    if text.contains("has no dm-") {
        return "本地包目录必须同时包含 `dm-plugin.toml` 和构建好的 `dm-<name>`：先 `cargo build --release --locked`，再把 `target/release/dm-<name>` 复制到清单同级后重试。".into();
    }
    if text.contains("already exists on disk") || text.contains("stale transaction") {
        return "磁盘上存在残留或未完成的事务；运行 `dm doctor --repair` 可自动修复。".into();
    }
    if text.contains("not writable") || text.contains("cannot open sqlite") {
        return "请检查 `DM_PLUGIN_HOME` 目录是否存在且可写，或设置 `DM_PLUGIN_HOME` 指向可写目录。".into();
    }
    if text.contains("plugin_environment") || text.contains("plugin.environment") {
        return "`[plugin] environment` 只能填合法的环境变量名，例如 [\"DM_DATABASE_URL\"]；`DM_PLUGIN_ENVIRONMENT` 用逗号分隔。".into();
    }
    if text.contains("dm_progress") {
        return "`DM_PROGRESS` 只接受 true/false（也支持 1/0、on/off、yes/no）。".into();
    }
    if text.contains("not published for target") {
        return "请让 `--target`、`DM_UPDATE_TARGET` 或 config.toml 中 `[update] target` 的值取上面列出的已发布目标。".into();
    }
    if text.contains("unknown field")
        && [
            "`log`",
            "`update_repository`",
            "`update_target`",
            "`progress`",
            "`plugin_environment`",
        ]
        .iter()
        .any(|key| text.contains(key))
    {
        return "配置已按用途分表：请改用 `[log] level`、`[update] repository/target`、`[output] progress`、`[plugin] environment`，示例见 examples/config.toml。".into();
    }
    if text.contains("config.toml") {
        return "请检查 `<DM_PLUGIN_HOME>/config.toml` 的表名、键名与 TOML 语法（可对照仓库中的 examples/config.toml），或删除该文件改用默认值。".into();
    }
    if text.contains("owner/repository form") {
        return "请把 `DM_UPDATE_REPOSITORY` 或 `config.toml` 中的 `update_repository` 改成 `owner/repository` 形式。".into();
    }
    if text.contains("sha-256") || text.contains("checksum") {
        return "校验失败通常表示文件损坏或被篡改；请重新下载，或联系插件/宿主发布者。".into();
    }
    if text.contains("prebuilt") {
        return "请确认 Release 提供了当前平台的 `dm-<name>-<target>` 预编译二进制及其 `.sha256` 侧车。".into();
    }
    if text.contains("unsupported plugin api") || text.contains("api_version") {
        return "插件要求的 API 版本与当前 `dm` 不兼容；请升级宿主或插件。".into();
    }
    if text.contains("source must be") || text.contains("local plugin directory") {
        return "`<source>` 需为已存在的本地插件目录，或以 `https://` 开头的 Git 仓库地址。".into();
    }
    if text.contains("git") || text.contains("curl") || text.contains("https") {
        return "请确认已安装 `git`/`curl`、网络可达，且插件来源为 HTTPS Git 仓库。".into();
    }
    if text.contains("manifest") {
        return "请检查 `dm-plugin.toml` 是否存在、为合法 TOML，并满足名称/版本/API 校验。".into();
    }
    if text.contains("self-update") || text.contains("release") || text.contains("target") {
        return "自更新失败；请确认发布资产与 SHA-256 校验文件完整。".into();
    }
    if text.contains("plugin name") || text.contains("reserved") {
        return "插件名需为 1–64 位小写字母、数字或 `-`，且不能使用内置命令保留名。".into();
    }

    "运行 `dm --help` 查看用法，或运行 `dm doctor` 检查插件存储状态。".into()
}
