use jiff::{Span, civil::Date};
use std::{
    fs::{self, File},
    io::{self, BufRead, BufReader, Seek, SeekFrom, Write},
    path::Path,
};

pub(super) fn cleanup(directory: &Path, today: Date) -> io::Result<()> {
    let cutoff = today
        .checked_sub(Span::new().days(29))
        .map_err(io::Error::other)?;
    for entry in fs::read_dir(directory)? {
        let Ok(entry) = entry else { continue };
        if !entry.file_type().is_ok_and(|kind| kind.is_file()) {
            continue;
        }
        let name = entry.file_name();
        let Some(date) = name
            .to_str()
            .and_then(|name| name.strip_prefix("dm-"))
            .and_then(|name| name.strip_suffix(".log"))
            .and_then(|name| name.parse::<Date>().ok())
        else {
            continue;
        };
        if date < cutoff {
            let _ = fs::remove_file(entry.path());
        }
    }
    Ok(())
}

pub(super) fn compact(path: &Path, keep: u64) -> io::Result<()> {
    let mut source = File::open(path)?;
    source.seek(SeekFrom::Start(
        source.metadata()?.len().saturating_sub(keep),
    ))?;
    let mut temporary = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
    // Copying is bounded; never read the whole log into memory.
    // Discard a partial old line at the cut point.
    let mut source = BufReader::with_capacity(8192, source);
    source.skip_until(b'\n')?;
    io::copy(&mut source, &mut temporary)?;
    temporary.flush()?;
    drop(source); // Windows requires the old file to be closed before replacement.
    temporary.persist(path).map_err(|error| error.error)?;
    Ok(())
}
