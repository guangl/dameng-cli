//! Hexadecimal encoding for the checksums the host compares and prints.

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
