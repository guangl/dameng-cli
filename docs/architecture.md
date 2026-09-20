# 架构

```text
用户 -> dm (CLI)
          ├─ install -> SQLite 名称注册表 / 本地路径 / HTTPS Git
          │             -> Rust crate + API 校验
          │             -> Cargo locked release build
          │             -> 临时目录校验 -> 原子重命名
          ├─ list / uninstall -> 本地插件存储
          └─ <plugin> [args] -> Rust 插件独立进程 -> 数据库工具逻辑
                                  └─ dm-plugin-sdk
```

## 模块职责

| 模块 | 责任 |
| --- | --- |
| `src/main.rs` | 参数解析、内置命令、外部子命令路由、错误展示 |
| `src/manifest.rs` | 严格清单解析、名称限制、API 版本、固定入口命名 |
| `src/store.rs` | SQLite 元数据、源解析、Rust crate 校验、编译安装、列举、卸载、进程调用 |
| `crates/dm-plugin-sdk` | `Plugin` / `Context` / `PluginResult` 和协议版本 |
| `examples/hello` | 唯一演示插件，验证 SDK 使用方法 |
| `tests/plugins.rs` | 真实 Rust crate 安装和进程协议回归测试 |

## 安装事务

插件只能从 Rust 源码安装，必须显式依赖 `dm-plugin-sdk` 并声明 `dm-<name>` binary target。
本地源码不复制，远程源码浅克隆到临时目录；宿主通过 Cargo 编译到插件存储内的独立临时目录，显式指定宿主 target，避免用户默认交叉编译目标导致安装错误产物。
只将清单与编译后的可执行文件装入最终目录；源文件、Git 元数据和构建缓存不会进入安装结果。资源应通过 Rust 的 `include_str!` / `include_bytes!` 嵌入。
编译失败时清理临时目录；成功后使用同文件系统目录重命名发布，再将经过校验的清单写入 SQLite。数据库写入失败时回滚刚发布的插件目录。拒绝同名覆盖，并发安装只有一个成功；进程被强制杀死时可能留下隐藏的 `.install-*` 临时目录，可在确认没有安装任务后删除。
不支持安装过程中修改源码或同时卸载正在运行的插件。

```text
DM_HOME/
├── store.sqlite3              # 插件元数据和名称 -> HTTPS Git 注册表
├── store.sqlite3-wal          # SQLite 运行时文件，存在时不要单独移动
├── store.sqlite3-shm          # SQLite 运行时文件，存在时不要单独移动
└── plugins/                   # 可执行文件不能存入 SQLite 后直接运行
    └── hello/
        ├── dm-plugin.toml
        └── dm-hello[.exe]
```

## 运行边界

SDK 使用 Rust trait 统一开发接口；跨进程只约定参数、环境变量、标准输入输出和退出码，不共享 Rust 内存布局。安装 API 和运行 SDK API 都检查兼容性。
宿主不连接数据库，不引入数据库 SDK，不维护全局连接或业务命令。
独立进程提供故障隔离，但不是权限沙箱。Cargo 构建脚本和插件拥有当前用户权限。SDK 依赖声明用于开发契约校验，不构成来源认证。
插件需要的数据库客户端动态库由插件作者声明和管理；宿主不会自动打包动态库。

## 后续扩展位置

源解析集中在 `PluginStore::install`，名称注册表通过 `dm registry` 管理并与已安装插件元数据共同存入 SQLite。后续可以接入受维护的远程注册表、固定 Git revision、签名和二进制分发。首版只获取远程默认分支，`Cargo.lock` 固定依赖但不固定远程插件源码；需要固定版本时在本地检出对应提交再安装。
业务能力始终在独立 Rust 插件仓库实现，不向宿主添加数据库业务子命令。
