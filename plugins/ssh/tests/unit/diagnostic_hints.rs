use dm_plugin_ssh::ssh_hint;
#[test]
fn ssh_failure_hints_distinguish_network_authentication_and_local_setup() {
    for (message, hint) in [
        ("Permission denied (publickey)", "认证失败"),
        ("Host key verification failed", "主机密钥"),
        ("REMOTE HOST IDENTIFICATION HAS CHANGED", "主机密钥"),
        ("Could not resolve hostname example", "无法解析"),
        ("Connection refused", "网络连接失败"),
        ("Connection timed out", "网络连接失败"),
        (
            "启动 SSH 测试失败；运行 dm ssh doctor 检查工具与配置",
            "dm ssh doctor",
        ),
        ("连接 'prod' 不存在", "dm ssh add"),
    ] {
        assert!(ssh_hint(&anyhow::anyhow!(message)).contains(hint));
    }
}
