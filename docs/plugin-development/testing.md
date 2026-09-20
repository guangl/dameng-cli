# 测试与调试

## 分层测试

推荐把测试分为三层：

1. 业务单元测试：直接测试参数解析、SQL 生成和数据库操作封装。
2. 插件进程测试：验证标准流、退出码和错误消息。
3. 宿主生命周期测试：真实执行安装、列举、调用和卸载。

## 基础质量检查

在插件仓库中运行：

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
cargo doc --no-deps --locked
```

CI 应在插件支持的每个操作系统上运行测试，并使用提交的 `Cargo.lock`。

## 真实安装测试

使用隔离的数据目录，避免覆盖日常安装：

```sh
plugin_test_home=$(mktemp -d)
DM_HOME="$plugin_test_home" dm install .
DM_HOME="$plugin_test_home" dm list
DM_HOME="$plugin_test_home" dm backup --help
DM_HOME="$plugin_test_home" dm uninstall backup
```

检查以下行为：

- 构建失败后没有可见的半安装插件。
- 同名重复安装被拒绝。
- 删除源码目录后插件仍能运行。
- 参数中的空格、`--` 和 `--help` 保持原样。
- stdin、stdout、stderr 和非零退出码符合文档。
- 错误信息不泄漏密码或连接串。

## 调试建议

插件 binary 不能脱离宿主直接运行。需要调试器时，先通过 `dm` 确认协议行为，再让调试配置提供与宿主相同的三个环境变量。不要把伪造协议变量的命令写成面向用户的正式启动方式。

如需查看安装结果，插件目录位于 `DM_HOME/plugins/<name>`。该目录是宿主管理区域，不要手工修改；修改会导致清单校验失败。

下一步阅读[发布与分发](publishing.md)。

