//! `PluginStore` error reporting and configuration settings.

use dameng_cli::{PluginStore, progress_bar_for};

#[test]
fn load_reports_missing_plugin() {
    let temp = tempfile::tempdir().unwrap();
    let store = PluginStore::new(temp.path());
    let error = store.load("missing").unwrap_err();
    assert!(error.to_string().contains("not installed"), "{error:#}");
}
#[test]
fn store_applies_the_configuration_settings() {
    let temp = tempfile::tempdir().unwrap();
    let forced = PluginStore::new(temp.path())
        .with_progress(Some(true))
        .with_plugin_environment(vec!["DM_TEST_VALUE".to_owned()]);
    assert!(forced.progress_enabled());
    assert!(
        !PluginStore::new(temp.path())
            .with_progress(Some(false))
            .progress_enabled()
    );

    // Without an explicit setting the terminal decides, as before.
    use std::io::IsTerminal;
    assert_eq!(
        PluginStore::new(temp.path()).progress_enabled(),
        std::io::stderr().is_terminal()
    );
}
#[test]
fn progress_bar_for_supports_terminal_and_hidden() {
    assert!(progress_bar_for(8, false).is_hidden());
    drop(progress_bar_for(8, true));
}
