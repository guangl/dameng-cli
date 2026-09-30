use dameng_cli::logging::{DailyLogWriter, daily_file_name};
use jiff::civil::Date;
use std::{
    fs,
    io::Write,
    sync::{Arc, Mutex},
};
use tempfile::TempDir;

fn date(value: &str) -> Date {
    value.parse().unwrap()
}

#[test]
fn daily_rollover_cleans_only_logs_older_than_thirty_days() {
    let temp = TempDir::new().unwrap();
    for name in [
        "dm-2026-08-31.log",
        "dm-2026-09-01.log",
        "notes.log",
        "dm-invalid.log",
    ] {
        fs::write(temp.path().join(name), b"history\n").unwrap();
    }
    let clock = Arc::new(Mutex::new(date("2026-09-30")));
    let copy = clock.clone();
    let mut writer =
        DailyLogWriter::with_clock(temp.path(), 64, move || *copy.lock().unwrap()).unwrap();
    writer.write_all(b"today\n").unwrap();
    assert!(!temp.path().join("dm-2026-08-31.log").exists());
    assert!(temp.path().join("dm-2026-09-01.log").exists());
    *clock.lock().unwrap() = date("2026-10-01");
    writer.write_all(b"tomorrow\n").unwrap();
    assert!(!temp.path().join("dm-2026-09-01.log").exists());
    assert_eq!(
        fs::read(temp.path().join("dm-2026-09-30.log")).unwrap(),
        b"today\n"
    );
    assert_eq!(
        fs::read(temp.path().join("dm-2026-10-01.log")).unwrap(),
        b"tomorrow\n"
    );
    for name in ["notes.log", "dm-invalid.log"] {
        assert!(temp.path().join(name).exists());
    }
}

#[test]
fn size_is_bounded_during_writes_and_oversized_records_do_not_create_extra_files() {
    let temp = TempDir::new().unwrap();
    let day = date("2026-09-30");
    let path = temp.path().join(daily_file_name(day));
    let mut writer = DailyLogWriter::with_clock(temp.path(), 64, move || day).unwrap();
    for index in 0..40 {
        writer
            .write_all(format!("entry {index:02}\n").as_bytes())
            .unwrap();
        assert!(path.metadata().unwrap().len() <= 64);
    }
    assert!(fs::read_to_string(&path).unwrap().ends_with("entry 39\n"));
    writer.write_all(&[b'x'; 200]).unwrap();
    assert_eq!(path.metadata().unwrap().len(), 64);
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 2); // log + lock
}

#[test]
fn concurrent_writers_keep_every_record_and_share_the_daily_file() {
    let temp = TempDir::new().unwrap();
    let mut threads = Vec::new();
    for id in 0..4 {
        let directory = temp.path().to_path_buf();
        threads.push(std::thread::spawn(move || {
            let mut writer =
                DailyLogWriter::with_clock(&directory, 8192, || date("2026-09-30")).unwrap();
            for index in 0..30 {
                writer
                    .write_all(format!("{id}:{index}\n").as_bytes())
                    .unwrap();
            }
        }));
    }
    for thread in threads {
        thread.join().unwrap();
    }
    let contents = fs::read_to_string(temp.path().join("dm-2026-09-30.log")).unwrap();
    assert_eq!(contents.lines().count(), 120);
    for id in 0..4 {
        for index in 0..30 {
            assert!(contents.lines().any(|line| line == format!("{id}:{index}")));
        }
    }
}

#[test]
fn unusable_destinations_and_write_failures_do_not_escape_the_writer() {
    let temp = TempDir::new().unwrap();
    assert!(DailyLogWriter::new(temp.path(), 0).is_err());
    let blocked = temp.path().join("blocked");
    fs::write(&blocked, b"file").unwrap();
    assert!(DailyLogWriter::new(&blocked, 64).is_err());
    fs::create_dir(temp.path().join("dm-2026-09-30.log")).unwrap();
    let mut writer = DailyLogWriter::with_clock(temp.path(), 64, || date("2026-09-30")).unwrap();
    writer.write_all(b"discard silently\n").unwrap();
    writer.flush().unwrap();
}

#[cfg(unix)]
#[test]
fn symlinks_are_not_followed_as_log_files() {
    let temp = TempDir::new().unwrap();
    let destination = temp.path().join("output");
    fs::write(&destination, b"untouched").unwrap();
    std::os::unix::fs::symlink(&destination, temp.path().join("dm-2026-09-30.log")).unwrap();
    let mut writer = DailyLogWriter::with_clock(temp.path(), 64, || date("2026-09-30")).unwrap();
    writer.write_all(b"diagnostic\n").unwrap();
    assert_eq!(fs::read(destination).unwrap(), b"untouched");
}
