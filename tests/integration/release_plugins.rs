//! Exercise durable Release sources with actual archives and offline HTTP fixtures.
#[cfg(unix)]
mod unix {
    use crate::common::*;
    use std::os::unix::fs::PermissionsExt;
    use std::{fs, path::Path, process::Command};
    use tempfile::TempDir;
    fn curl(root: &Path, assets: &Path) -> std::ffi::OsString {
        let bin = root.join("bin");
        fs::create_dir_all(&bin).unwrap();
        fs::write(
            bin.join("curl"),
            r#"#!/bin/sh
if [ "${DM_CURL_FAIL:-}" = yes ]; then echo 'network unavailable' >&2; exit 22; fi
output=''
while [ "$#" -gt 0 ]; do
    case "$1" in
        --output) shift; output=$1 ;;
        https://*) url=$1 ;;
    esac
    shift
done
case "$url" in
    */releases/latest) echo 'https://github.com/test/repo/releases/tag/v0.3.1' ;;
    *) file=${url##*/}; cp "$DM_RELEASE_FIXTURE/$file" "$output" ;;
esac
"#,
        )
        .unwrap();
        fs::set_permissions(bin.join("curl"), fs::Permissions::from_mode(0o755)).unwrap();
        let mut paths = vec![bin];
        paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap()));
        assert!(assets.is_dir());
        std::env::join_paths(paths).unwrap()
    }
    fn assets(root: &Path) -> std::path::PathBuf {
        let target = env!("DM_HOST_TARGET");
        let folder = format!("dm-probe-v0.3.1-{target}");
        let package = root.join(&folder);
        fs::create_dir(&package).unwrap();
        fs::write(
            package.join("dm-plugin.toml"),
            format!(
                "{}\n[hooks]\npost_install = 'hooks/ready'\n",
                manifest("probe").replace("0.1.0", "0.2.0")
            ),
        )
        .unwrap();
        fs::write(package.join("dm-probe"), "#!/bin/sh\necho updated\n").unwrap();
        fs::create_dir(package.join("hooks")).unwrap();
        fs::write(package.join("hooks/ready"), "#!/bin/sh\nexit 0\n").unwrap();
        let asset = root.join(format!("{folder}.tar.gz"));
        assert!(
            Command::new("tar")
                .arg("-czf")
                .arg(&asset)
                .arg("-C")
                .arg(root)
                .arg(folder)
                .status()
                .unwrap()
                .success()
        );
        use sha2::{Digest, Sha256};
        let checksum = format!(
            "{}\n",
            dameng_cli::support::codec::hex(&Sha256::digest(fs::read(&asset).unwrap()))
        );
        fs::write(asset.with_extension("gz.sha256"), checksum).unwrap();
        root.to_path_buf()
    }
    #[test]
    fn recorded_release_sources_support_check_update_and_preserve_data() {
        let temp = TempDir::new().unwrap();
        let assets_root = temp.path().join("assets");
        fs::create_dir(&assets_root).unwrap();
        let assets = assets(&assets_root);
        let path = curl(temp.path(), &assets);
        let source = fixture(temp.path());
        let home = temp.path().join("home");
        let run = |args: &[&str]| {
            dm(&home)
                .env("PATH", &path)
                .env("DM_RELEASE_FIXTURE", &assets)
                .args(args)
                .output()
                .unwrap()
        };
        ok(run(&[
            "install",
            source.to_str().unwrap(),
            "--release-source",
            "test/repo",
            "--release-tag",
            "v0.3.0",
        ]));
        fs::create_dir_all(home.join("data/probe")).unwrap();
        fs::write(home.join("data/probe/connection"), "keep").unwrap();
        let check: serde_json::Value =
            serde_json::from_str(&ok(run(&["update", "--json"]))).unwrap();
        assert_eq!(check[0]["available_version"], "0.2.0");
        assert_eq!(check[0]["update_available"], true);
        ok(run(&["update", "probe"]));
        assert_eq!(
            fs::read_to_string(home.join("data/probe/connection")).unwrap(),
            "keep"
        );
        let info: serde_json::Value =
            serde_json::from_str(&ok(run(&["info", "probe", "--json"]))).unwrap();
        assert_eq!(info["source"], "github-release:test/repo");
        assert_eq!(info["revision"], "v0.3.1");
        assert_eq!(info["manifest"]["version"], "0.2.0");
        assert!(home.join("plugins/probe/hooks/ready").is_file());
        assert!(
            !dm(&home)
                .env("PATH", &path)
                .env("DM_CURL_FAIL", "yes")
                .args(["update", "probe"])
                .output()
                .unwrap()
                .status
                .success()
        );
        let target = env!("DM_HOST_TARGET");
        fs::write(
            assets.join(format!("dm-probe-v0.3.1-{target}.tar.gz.sha256")),
            "0".repeat(64),
        )
        .unwrap();
        let output = run(&["update", "probe"]);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("SHA-256 mismatch"));
        assert_eq!(
            fs::read_to_string(home.join("data/probe/connection")).unwrap(),
            "keep"
        );
        assert!(
            !run(&[
                "install",
                source.to_str().unwrap(),
                "--release-source",
                "bad",
                "--release-tag",
                "v0.3.0"
            ])
            .status
            .success()
        );
        assert!(
            !run(&[
                "install",
                source.to_str().unwrap(),
                "--release-source",
                "test/repo",
                "--release-tag",
                "v0.3.0",
                "--release-target",
                "invalid"
            ])
            .status
            .success()
        );
    }
}
