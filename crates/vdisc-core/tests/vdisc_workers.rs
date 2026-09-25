#![cfg(target_os = "linux")]
#[path = "support/vdisc_fixture.rs"]
mod fixture;
use std::{
    fs,
    io::Write,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
use vdisc_core::format::{FormatErrorKind, IMAGE_MEMORY_BUDGET, inspect_artwork, validate_vdisc};

#[test]
fn workers_enforce_memory_and_cpu_limits_before_reading_input() {
    for worker in [
        env!("CARGO_BIN_EXE_vdisc-image-check"),
        env!("CARGO_BIN_EXE_vdisc-media-check"),
    ] {
        let input = tempfile::NamedTempFile::new().unwrap();
        let mut child = Command::new(worker)
            .args([input.path().to_str().unwrap(), "0", "0"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        let passed = loop {
            let limits = fs::read_to_string(format!("/proc/{}/limits", child.id())).unwrap();
            let memory = limits
                .lines()
                .find(|l| l.starts_with("Max address space"))
                .unwrap()
                .split_whitespace()
                .collect::<Vec<_>>();
            let cpu = limits
                .lines()
                .find(|l| l.starts_with("Max cpu time"))
                .unwrap()
                .split_whitespace()
                .collect::<Vec<_>>();
            if memory[3] == IMAGE_MEMORY_BUDGET.to_string()
                && memory[4] == IMAGE_MEMORY_BUDGET.to_string()
                && cpu[3] == "10"
                && cpu[4] == "10"
            {
                break true;
            }
            if Instant::now() >= deadline {
                break false;
            }
            std::thread::sleep(Duration::from_millis(5));
        };
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(passed, "worker limits were not installed");
    }
}

#[test]
fn abnormal_worker_exit_returns_controlled_resource_errors() {
    const FLAG: &str = "VDISC_TEST_WORKER_FAILURE_CHILD";
    if std::env::var_os(FLAG).is_some() {
        assert_eq!(
            inspect_artwork(b"image").unwrap_err().kind,
            FormatErrorKind::ResourceLimit
        );
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(&fixture::build(&fixture::minimal_entries(), false, false))
            .unwrap();
        assert_eq!(
            validate_vdisc(file.path()).unwrap_err().kind,
            FormatErrorKind::ResourceLimit
        );
        return;
    }
    // Per-child environment avoids races with concurrent tests and leaves the
    // parent environment unchanged. /bin/false simulates a worker that cannot run.
    let status = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "abnormal_worker_exit_returns_controlled_resource_errors",
        ])
        .env(FLAG, "1")
        .env("VDISC_IMAGE_WORKER", "/bin/false")
        .env("VDISC_MEDIA_WORKER", "/bin/false")
        .status()
        .unwrap();
    assert!(status.success());
}
