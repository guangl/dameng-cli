# 贡献指南

欢迎通过 Issue 讨论问题和扩展方案，再提交范围清晰的 Pull Request。

所有仓库修改都通过功能分支和 Pull Request 进入 `main`；普通 CI 在 PR 上运行，发布仍由 `v*` 标签触发。请不要直接推送 `main`。

## 范围

本仓库维护 `dm` 宿主、Rust 插件 SDK、协议与基础示例。数据库连接、SQL 工具、导入导出、巡检等业务功能应建立独立 Rust 插件，不添加到宿主。

## 本地验证

需要稳定版 Rust、Cargo 和 Git。

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo doc --workspace --no-deps --locked
git diff --check
```

覆盖率不低于 95%（按行统计），CI 会用同一条命令把关，本地需要 `cargo install cargo-llvm-cov` 和 `rustup component add llvm-tools-preview`：

```sh
cargo llvm-cov --workspace --locked --fail-under-lines 95
cargo llvm-cov --workspace --locked --html   # 需要逐行定位未覆盖代码时
```

新增行为必须带测试；只被 `#[cfg]` 或故障注入才能触发的分支不计入要求，但要在 PR 说明中说明原因。

测试使用临时 `DM_PLUGIN_HOME` 和真实 Rust 测试插件，不需要达梦实例。集成测试离线构建无第三方依赖的测试插件；先完成一次宿主依赖下载。

提交前更新 README、CLI/协议/架构文档、示例和 CHANGELOG 中所有受影响部分。协议变更必须明确兼容性，影响安装、hook、参数透传、升级/回滚或失败恢复时补充对应测试。保持 Cargo.lock 受版本控制。

PR 说明应包含问题、行为变化、验证结果及限制。不要提交数据库凭证、构建产物或与当前任务无关的修改。提交贡献即表示你有权以本仓库 MIT 许可证提供这些内容。
