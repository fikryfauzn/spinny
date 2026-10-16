//! Background insertion and the independent mechanical seating handshake.
#![cfg(target_os = "linux")]

use std::{
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};
use vdisc_appliance::{DiscState, LidState};
use vdisc_qml_bridge::{ApplianceRuntime, Command};

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/vdisc/valid-v1.vdisc")
}

fn open_lid(rt: &mut ApplianceRuntime) {
    rt.execute(Command::Open).unwrap();
    rt.execute(Command::LidOpened).unwrap();
    assert_eq!(rt.controller().lid_state(), LidState::Open);
}

fn complete_validation(rt: &mut ApplianceRuntime) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        if let Some(result) = rt.poll_insertion() {
            return result;
        }
        assert!(Instant::now() < deadline, "validation timed out");
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn animation_first_does_not_seat_until_validated() {
    let mut rt = ApplianceRuntime::new();
    open_lid(&mut rt);
    rt.execute(Command::Insert(fixture())).unwrap();
    assert!(rt.insertion_pending());
    assert_eq!(rt.controller().disc_state(), DiscState::Inserting);
    assert!(rt.execute(Command::Close).is_err());
    assert!(rt.execute(Command::Play).is_err());
    rt.execute(Command::DiscInserted).unwrap();
    assert_eq!(rt.controller().disc_state(), DiscState::Inserting);
    complete_validation(&mut rt).unwrap();
    assert_eq!(rt.controller().disc_state(), DiscState::Seated);
    assert!(!rt.insertion_pending());
    assert!(rt.backend().track_count() > 0);
}

#[test]
fn validation_first_waits_for_animation_acknowledgement() {
    let mut rt = ApplianceRuntime::new();
    open_lid(&mut rt);
    rt.execute(Command::Insert(fixture())).unwrap();
    complete_validation(&mut rt).unwrap();
    assert_eq!(rt.controller().disc_state(), DiscState::Inserting);
    assert!(rt.execute(Command::Close).is_err());
    rt.execute(Command::DiscInserted).unwrap();
    assert_eq!(rt.controller().disc_state(), DiscState::Seated);
    assert!(rt.execute(Command::DiscInserted).is_err());
}

#[test]
fn missing_disc_fails_closed_and_can_retry() {
    let mut rt = ApplianceRuntime::new();
    open_lid(&mut rt);
    let missing =
        std::env::temp_dir().join(format!("vdisc-async-absent-{}.vdisc", std::process::id()));
    // A per-test PID suffix is sufficient since we never create this path.
    assert!(!missing.exists());
    rt.execute(Command::Insert(missing)).unwrap();
    assert!(rt.execute(Command::Insert(fixture())).is_err());
    assert!(complete_validation(&mut rt).is_err());
    assert_eq!(rt.controller().disc_state(), DiscState::Absent);
    assert!(!rt.insertion_pending());
    assert_eq!(rt.backend().track_count(), 0);
    rt.execute(Command::Insert(fixture())).unwrap();
    rt.execute(Command::DiscInserted).unwrap();
    complete_validation(&mut rt).unwrap();
    assert_eq!(rt.controller().disc_state(), DiscState::Seated);
}
