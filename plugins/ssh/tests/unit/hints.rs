use dm_plugin_ssh::ssh_hint;

#[test]
fn hints_cover_every_reported_failure_class() {
    let hint = |message: &str| ssh_hint(&anyhow::anyhow!(message.to_owned()));
    assert!(hint("SSH server 'prod' is not configured").contains("dm ssh add"));
    assert!(hint("password authentication requires sshpass").contains("sshpass"));
    assert!(hint("SSH password or key path is required").contains("--password"));
    assert!(hint("no such table: servers").contains("servers.sqlite3"));
    assert!(hint("SSH host is required").contains("必填项"));
    assert!(hint("something else entirely").contains("dm ssh --help"));
}
