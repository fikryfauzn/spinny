use std::path::{Path, PathBuf};

use vdisc_appliance::{DiscState, LidState, PlayMode, TransportState};
use vdisc_qml_bridge::{ApplianceRuntime, Command};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/vdisc/valid-v1.vdisc")
}

fn seated_runtime() -> ApplianceRuntime {
    let mut runtime = ApplianceRuntime::new();
    runtime.execute(Command::Open).unwrap();
    runtime.execute(Command::LidOpened).unwrap();
    runtime.execute(Command::Insert(fixture())).unwrap();
    runtime.execute(Command::DiscInserted).unwrap();
    runtime.execute(Command::Close).unwrap();
    runtime.execute(Command::LidClosed).unwrap();
    runtime
}

#[test]
fn starts_with_closed_empty_stopped_machine_at_half_volume() {
    let runtime = ApplianceRuntime::new();
    assert_eq!(runtime.controller().lid_state(), LidState::Closed);
    assert_eq!(runtime.controller().disc_state(), DiscState::Absent);
    assert_eq!(
        runtime.controller().transport_state(),
        TransportState::Stopped
    );
    assert_eq!(runtime.controller().volume().normalized(), 0.5);
}

#[test]
fn open_requires_completion_and_rejects_out_of_order_callbacks() {
    let mut runtime = ApplianceRuntime::new();
    assert!(runtime.execute(Command::LidClosed).is_err());
    assert_eq!(runtime.controller().lid_state(), LidState::Closed);

    runtime.execute(Command::Open).unwrap();
    assert_eq!(runtime.controller().lid_state(), LidState::Opening);
    runtime.execute(Command::LidOpened).unwrap();
    assert_eq!(runtime.controller().lid_state(), LidState::Open);
    assert!(runtime.execute(Command::Open).is_err());
    assert_eq!(runtime.controller().lid_state(), LidState::Open);
}

#[test]
fn hold_and_playing_interlock_reject_open_without_motion() {
    let mut runtime = ApplianceRuntime::new();
    runtime.execute(Command::SetHold(true)).unwrap();
    assert!(runtime.execute(Command::Open).is_err());
    assert_eq!(runtime.controller().lid_state(), LidState::Closed);
    runtime.execute(Command::SetHold(false)).unwrap();

    let mut runtime = seated_runtime();
    runtime.execute(Command::Play).unwrap();
    assert!(runtime.execute(Command::Open).is_err());
    assert_eq!(runtime.controller().lid_state(), LidState::Closed);
    assert_eq!(
        runtime.controller().transport_state(),
        TransportState::Playing
    );
}

#[test]
fn insertion_and_removal_wait_for_presentation_callbacks() {
    let mut runtime = ApplianceRuntime::new();
    runtime.execute(Command::Open).unwrap();
    runtime.execute(Command::LidOpened).unwrap();
    runtime.execute(Command::Insert(fixture())).unwrap();
    assert_eq!(runtime.controller().disc_state(), DiscState::Inserting);
    runtime.execute(Command::DiscInserted).unwrap();
    assert_eq!(runtime.controller().disc_state(), DiscState::Seated);
    assert_eq!(runtime.backend().track_count(), 1);
    runtime.execute(Command::Remove).unwrap();
    assert_eq!(runtime.controller().disc_state(), DiscState::Removing);
    runtime.execute(Command::DiscRemoved).unwrap();
    assert_eq!(runtime.controller().disc_state(), DiscState::Absent);
    assert_eq!(runtime.backend().track_count(), 0);
}

#[test]
fn missing_disc_path_reports_error_without_seating_disc() {
    let mut runtime = ApplianceRuntime::new();
    runtime.execute(Command::Open).unwrap();
    runtime.execute(Command::LidOpened).unwrap();
    assert!(runtime.execute(Command::Insert(PathBuf::new())).is_err());
    assert_eq!(runtime.controller().disc_state(), DiscState::Absent);
    assert!(
        runtime
            .execute(Command::Insert(PathBuf::from("/missing.vdisc")))
            .is_err()
    );
    assert_eq!(runtime.controller().disc_state(), DiscState::Absent);
    assert!(runtime.controller().error_state().is_some());
}

#[test]
fn invalid_numbers_do_not_mutate_volume_or_transport() {
    let mut runtime = ApplianceRuntime::new();
    for value in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
        assert!(runtime.execute(Command::SetVolume(value)).is_err());
        assert_eq!(runtime.controller().volume().normalized(), 0.5);
    }
    assert!(runtime.execute(Command::ScanStep(-1)).is_err());
    assert_eq!(
        runtime.controller().transport_state(),
        TransportState::Stopped
    );
}

#[test]
fn transport_navigation_menu_and_scan_use_existing_machine() {
    let mut runtime = seated_runtime();
    runtime.execute(Command::Play).unwrap();
    runtime.execute(Command::MenuShort).unwrap();
    assert_eq!(runtime.controller().play_mode(), PlayMode::RepeatAll);
    runtime
        .execute(Command::ScanBegin(vdisc_appliance::ScanDirection::Forward))
        .unwrap();
    assert_eq!(
        runtime.controller().transport_state(),
        TransportState::SeekingForward
    );
    runtime.execute(Command::ScanStep(1)).unwrap();
    runtime.execute(Command::ScanEnd).unwrap();
    runtime.execute(Command::Previous).unwrap();
    assert!(runtime.execute(Command::Next).is_err());
    assert_eq!(
        runtime.controller().transport_state(),
        TransportState::Playing
    );
    runtime.execute(Command::Pause).unwrap();
    assert_eq!(
        runtime.controller().transport_state(),
        TransportState::Paused
    );
    runtime.execute(Command::Pause).unwrap();
    runtime.execute(Command::Stop).unwrap();
    assert_eq!(
        runtime.controller().transport_state(),
        TransportState::Stopped
    );
    runtime.execute(Command::MenuLong).unwrap();
    assert!(runtime.controller().avls_enabled());
    runtime.execute(Command::ToggleAvls).unwrap();
    assert!(!runtime.controller().avls_enabled());
}

#[test]
fn playing_without_disc_rejects_before_backend_changes() {
    let mut runtime = ApplianceRuntime::new();
    assert!(runtime.execute(Command::Play).is_err());
    assert_eq!(
        runtime.controller().transport_state(),
        TransportState::Stopped
    );
}
