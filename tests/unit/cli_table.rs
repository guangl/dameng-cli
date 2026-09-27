//! Unit tests for the `dm list` table renderer.

use dameng_cli::cli::table::{format_installed_at, render};
use dameng_cli::{API_VERSION, Manifest, PluginInfo};

fn plugin_info(
    name: &str,
    version: &str,
    description: &str,
    source: Option<&str>,
    revision: Option<&str>,
    installed_at: i64,
) -> PluginInfo {
    PluginInfo {
        manifest: Manifest {
            name: name.to_owned(),
            version: version.to_owned(),
            description: description.to_owned(),
            api_version: API_VERSION,
            min_host_version: None,
            license: None,
            homepage: None,
            environment: Vec::new(),
            hooks: Default::default(),
        },
        source: source.map(str::to_owned),
        revision: revision.map(str::to_owned),
        source_ref: None,
        checksum: String::new(),
        installed_at,
    }
}

#[test]
fn empty_table_renders_empty() {
    assert_eq!(render(&[]), "");
}

#[test]
fn table_contains_headers_and_rows() {
    let output = render(&[plugin_info(
        "probe",
        "0.1.0",
        "Test plugin",
        Some("/tmp/probe"),
        Some("deadbeef"),
        0,
    )]);

    for needle in [
        "Name",
        "Version",
        "Description",
        "Source",
        "Revision",
        "Installed At",
        "probe",
        "0.1.0",
        "Test plugin",
        "/tmp/probe",
        "deadbeef",
        "1970-01-01 00:00:00Z",
    ] {
        assert!(
            output.contains(needle),
            "missing {needle:?} in:
{output}"
        );
    }
}

#[test]
fn formats_epoch_seconds_as_utc() {
    assert_eq!(format_installed_at(0), "1970-01-01 00:00:00Z");
    assert_eq!(format_installed_at(951_782_400), "2000-02-29 00:00:00Z");
    assert_eq!(format_installed_at(-1), "1969-12-31 23:59:59Z");
}
