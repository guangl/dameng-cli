//! Hexadecimal encoding used by the existing secret and export formats.

use anyhow::{Result, ensure};

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn unhex(text: &str) -> Result<Vec<u8>> {
    ensure!(
        text.len().is_multiple_of(2) && text.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "Expected hexadecimal text"
    );
    (0..text.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&text[index..index + 2], 16).map_err(Into::into))
        .collect()
}
