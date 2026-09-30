use dm_plugin_support::{bounded, parallel, process};
use std::{
    io::{self, Read, Write},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

#[test]
fn bounded_reads_reject_excess_without_consuming_the_whole_stream() {
    let mut reader = io::Cursor::new(vec![b'x'; 1000]);
    assert!(bounded::read(&mut reader, 17).is_err());
    assert_eq!(reader.position(), 18);
    let mut next = [0; 1];
    reader.read_exact(&mut next).unwrap();
    assert_eq!(next, [b'x']);
    assert_eq!(bounded::read(&b"exact"[..], 5).unwrap(), b"exact");
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("input");
    std::fs::write(&path, [0xff]).unwrap();
    assert!(bounded::text(&path, 5).is_err());
    assert!(bounded::file(&temp.path().join("absent"), 5).is_err());
}

#[test]
fn worker_pool_bounds_concurrency_and_preserves_order() {
    let active = AtomicUsize::new(0);
    let peak = AtomicUsize::new(0);
    let result = parallel::map((0..30).collect(), 3, |item| {
        let running = active.fetch_add(1, Ordering::SeqCst) + 1;
        peak.fetch_max(running, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(5));
        active.fetch_sub(1, Ordering::SeqCst);
        item * 2
    })
    .unwrap();
    assert_eq!(result, (0..30).map(|item| item * 2).collect::<Vec<_>>());
    assert!((2..=3).contains(&peak.load(Ordering::SeqCst)));
    assert!(parallel::map(vec![1], 0, |item| item).is_err());
    assert!(parallel::map(vec![1], 1, |_| panic!("probe")).is_err());
    assert!(
        parallel::map(Vec::<u8>::new(), 4, |item| item)
            .unwrap()
            .is_empty()
    );
}

fn child(mode: &str) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--ignored",
            "--exact",
            "child_command_resource_probe",
            "--nocapture",
        ])
        .env("DM_RESOURCE_PROBE", mode);
    command
}

#[test]
fn captures_are_bounded_and_slow_helpers_are_terminated() {
    let output = process::capture(&mut child("normal"), Duration::from_secs(10)).unwrap();
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("probe stdout")
    );
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("probe stderr")
    );
    for mode in ["stdout", "stderr"] {
        assert_eq!(
            process::capture(&mut child(mode), Duration::from_secs(10))
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
    }
    assert_eq!(
        process::capture(&mut child("slow"), Duration::from_millis(100))
            .unwrap_err()
            .kind(),
        io::ErrorKind::TimedOut
    );
    assert!(
        process::capture(
            &mut Command::new("dm-no-such-command-resource-probe"),
            Duration::from_secs(1)
        )
        .is_err()
    );
}

#[test]
#[ignore = "subprocess helper used by the resource tests"]
fn child_command_resource_probe() {
    match std::env::var("DM_RESOURCE_PROBE").unwrap().as_str() {
        "normal" => {
            println!("probe stdout");
            eprintln!("probe stderr");
        }
        "stdout" => io::stdout().write_all(&[b'x'; 128 * 1024]).unwrap(),
        "stderr" => io::stderr().write_all(&[b'x'; 128 * 1024]).unwrap(),
        "slow" => std::thread::sleep(Duration::from_secs(30)),
        _ => unreachable!(),
    }
}
