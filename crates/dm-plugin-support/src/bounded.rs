//! Size bounds for data that must be parsed in memory.
use std::{
    fs::File,
    io::{self, Read},
    path::Path,
};

pub const CONFIG_LIMIT: u64 = 1024 * 1024;
pub const DOCUMENT_LIMIT: u64 = 16 * 1024 * 1024;

pub fn read(reader: impl Read, limit: u64) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Input exceeds {limit} bytes"),
        ));
    }
    Ok(bytes)
}

pub fn file(path: &Path, limit: u64) -> io::Result<Vec<u8>> {
    read(File::open(path)?, limit)
}

pub fn text(path: &Path, limit: u64) -> io::Result<String> {
    String::from_utf8(file(path, limit)?)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}
