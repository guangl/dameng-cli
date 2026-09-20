# dameng-cli 插件开发文档

这套文档面向准备开发、测试并分发 `dm` 插件的 Rust 开发者。插件是独立的 binary crate，通过 `dm-plugin-sdk` 接收宿主上下文，并以独立进程运行。

## 阅读路线

1. [快速开始](getting-started.md)：创建并运行第一个插件。
2. [项目结构与清单](manifest.md)：理解 `Cargo.toml`、`Cargo.lock` 和 `dm-plugin.toml` 的约束。
3. [SDK API](sdk-api.md)：使用 `Context`、`Plugin`、`PluginResult` 和入口函数。
4. [运行时协议](runtime-contract.md)：参数、环境变量、标准流和退出码的准确约定。
5. [测试与调试](testing.md)：从业务单元测试到真实安装生命周期测试。
6. [发布与分发](publishing.md)：准备独立仓库并支持本地、Git URL 和名称安装。
7. [故障排查](troubleshooting.md)：定位常见的清单、构建、安装和运行错误。

## 核心边界

- 宿主负责安装、发现、调用和卸载，不提供数据库连接或业务 API。
- 插件自行选择数据库驱动、参数解析器、日志库和配置格式。
- 宿主与插件不共享 Rust ABI；公共契约只有进程参数、环境变量、标准流和退出码。
- 安装会编译插件源码和构建脚本，用户只应安装可信仓库。

## 参考资料

- [插件协议 v1](../plugins.md)
- [宿主架构](../architecture.md)
- [可运行 hello 示例](../../examples/hello/)
- [SDK 源码](../../crates/dm-plugin-sdk/src/lib.rs)

