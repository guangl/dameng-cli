//! `Manifest` parsing, consent and release tag selection.

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
fn requests_consent_from_none_requires_declared_requests() {
    let empty = Manifest::from_toml(&base_manifest()).unwrap();
    assert!(!empty.requests_consent_from(None));

    let requesting = Manifest::from_toml(&format!(
        r#"{}
permissions = ["network"]
environment = ["DM_TOKEN"]
"#,
        base_manifest()
    ))
    .unwrap();
    assert!(requesting.requests_consent_from(None));
}
#[test]
fn requests_consent_compares_previous_declarations() {
    let previous = Manifest::from_toml(&format!(
        r#"{}
permissions = ["filesystem"]
environment = ["DM_HOME"]
"#,
        base_manifest()
    ))
    .unwrap();

    let same = Manifest::from_toml(&format!(
        r#"{}
permissions = ["filesystem"]
environment = ["DM_HOME"]
"#,
        base_manifest()
    ))
    .unwrap();
    assert!(!same.requests_consent_from(Some(&previous)));

    let new_permission = Manifest::from_toml(&format!(
        r#"{}
permissions = ["network"]
"#,
        base_manifest()
    ))
    .unwrap();
    assert!(new_permission.requests_consent_from(Some(&previous)));

    let new_environment = Manifest::from_toml(&format!(
        r#"{}
environment = ["DM_SECRET"]
"#,
        base_manifest()
    ))
    .unwrap();
    assert!(new_environment.requests_consent_from(Some(&previous)));
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
