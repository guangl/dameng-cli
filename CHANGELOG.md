# Changelog

## Unreleased

- 增加宿主 `dm self-update`，从 GitHub Release 下载目标平台归档、验证 SHA-256 并原子替换当前程序。
- `dm self-update` 增加 minisign 签名强制校验；Release 流程对归档生成签名。
- 增加远程 registry 索引支持：`dm registry sync <url>` 与 `dm search --remote <url>`。
- Release 产物新增 Linux ARM64（aarch64）与 x86_64 musl 静态目标。
- 增加插件原子更新、更新检查、启停、来源/revision/校验和溯源、完整性验证和故障回滚。
- 增加 `info`、`search`、`outdated`、`verify`、`doctor`、JSON 输出和 shell completion。
- 增加 `dm new` 插件项目脚手架。
- 增加每插件配置/数据/缓存目录、运行时能力协商和环境变量白名单。
- 扩展严格插件清单，支持最低宿主版本、许可证、主页、环境变量与声明式权限。
- 修复允许安装 `registry` 等内置命令同名插件、导致插件无法调用的问题。
- SQLite schema 升级至 v2，支持旧数据库迁移和损坏/中断状态修复。
- 将插件元数据和名称注册表迁移到 SQLite，并增加 `dm registry` 管理命令。
- 增加带 Release 校验的远程安装脚本和从当前源码构建的本地安装脚本。
- 增加 GitHub Pages 插件开发站点，覆盖项目结构、SDK、运行协议、测试、发布和故障排查。
- 增加 `dm` Rust 插件宿主：安装、列举、命令转发和卸载。
- 增加本地源码、HTTPS Git 仓库和可配置名称注册表来源。
- 增加 Rust SDK、版本化进程协议和 hello 示例。
- 增加原子安装、严格清单校验和失败清理。
- 增加跨平台测试、CI、版本标签发布流程和社区协作文件。
