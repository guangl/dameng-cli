use crate::manifest::validate_name;
use anyhow::{Context, Result, ensure};
use std::{fs, path::Path};

pub fn scaffold_plugin(name: &str, destination: &Path) -> Result<()> {
    validate_name(name)?;
    ensure!(
        !destination.exists(),
        "Destination {} already exists",
        destination.display()
    );
    fs::create_dir_all(destination.join("src"))
        .with_context(|| format!("Create {}", destination.display()))?;
    let result = write_project(name, destination);
    if result.is_err() {
        let _ = fs::remove_dir_all(destination);
    }
    result
}

fn write_project(name: &str, destination: &Path) -> Result<()> {
    let version = env!("CARGO_PKG_VERSION");
    fs::write(
        destination.join("Cargo.toml"),
        format!(
            "[package]\nname = \"dm-plugin-{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\nrust-version = \"1.85\"\n\n[[bin]]\nname = \"dm-{name}\"\npath = \"src/main.rs\"\n\n[dependencies]\ndm-plugin-sdk = {{ git = \"https://github.com/guangl/dameng-cli.git\", tag = \"v{version}\" }}\n"
        ),
    )?;
    fs::write(
        destination.join("dm-plugin.toml"),
        format!(
            "name = \"{name}\"\nversion = \"0.1.0\"\ndescription = \"A Dameng database plugin\"\napi_version = 1\nmin_host_version = \"{version}\"\npermissions = []\nenvironment = []\n"
        ),
    )?;
    fs::write(
        destination.join("src/main.rs"),
        "use dm_plugin_sdk::{Context, Plugin, PluginResult};\n\nstruct Tool;\n\nimpl Plugin for Tool {\n    fn run(&self, context: Context) -> PluginResult {\n        println!(\"received {} arguments\", context.args.len());\n        Ok(0)\n    }\n}\n\nfn main() {\n    dm_plugin_sdk::run(Tool);\n}\n",
    )?;
    fs::write(
        destination.join("README.md"),
        format!("# dm-plugin-{name}\n"),
    )?;
    fs::write(destination.join(".gitignore"), "/target\n")?;
    Ok(())
}
