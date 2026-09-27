//! `Manifest` parsing and release tag selection.

use dameng_cli::{Manifest, release_tag_candidates};

fn base_manifest() -> String {
    r#"
name = "demo"
version = "0.1.0"
description = "Demo plugin"
api_version = 1
"#
    .to_string()
}
#[test]
fn from_toml_accepts_satisfied_min_host_version() {
    let text = format!(
        r#"{}
min_host_version = "0.1.0"
"#,
        base_manifest()
    );
    let manifest = Manifest::from_toml(&text).unwrap();
    assert_eq!(manifest.min_host_version.as_deref(), Some("0.1.0"));
}
#[test]
fn from_toml_rejects_too_new_min_host_version() {
    let text = format!(
        r#"{}
min_host_version = "999.0.0"
"#,
        base_manifest()
    );
    assert!(Manifest::from_toml(&text).is_err());
}
#[test]
fn release_tag_candidates_prefers_revision() {
    let manifest = Manifest::from_toml(&base_manifest()).unwrap();
    assert_eq!(
        release_tag_candidates(&manifest, Some("v1.2.3")),
        vec!["v1.2.3", "v0.1.0"]
    );
    assert_eq!(
        release_tag_candidates(&manifest, Some("1.2.3")),
        vec!["v1.2.3", "v0.1.0"]
    );
    assert_eq!(release_tag_candidates(&manifest, None), vec!["v0.1.0"]);
}
