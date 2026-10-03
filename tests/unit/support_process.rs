use dameng_cli::support::process;
use std::{
    io::{self, Write},
    process::Command,
    time::Duration,
};

fn child(mode: &str) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--ignored",
            "--exact",
            "support_process::child_command_resource_probe",
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
