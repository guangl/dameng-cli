use anyhow::{Result, ensure};

/// Treat an unset or blank environment variable as "not configured".
pub(super) fn configured_value(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// Parse a boolean switch from an environment variable.
pub(super) fn parse_switch(name: &str, value: &str) -> Result<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        other => anyhow::bail!("{name} must be true or false, got '{other}'"),
    }
}

/// Validate environment variable names and drop duplicates while keeping order.
pub(super) fn valid_environment_names(names: &[String]) -> Result<Vec<String>> {
    let mut valid: Vec<String> = Vec::new();
    for name in names {
        let name = name.trim();
        ensure!(
            !name.is_empty(),
            "Configuration key 'plugin.environment' must not contain empty names"
        );
        ensure!(
            name.bytes().enumerate().all(|(index, byte)| byte == b'_'
                || byte.is_ascii_alphabetic()
                || (index > 0 && byte.is_ascii_digit())),
            "Configuration key 'plugin.environment' entry '{name}' is not a valid environment variable name"
        );
        if !valid.iter().any(|existing| existing == name) {
            valid.push(name.to_owned());
        }
    }
    Ok(valid)
}
