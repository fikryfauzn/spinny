use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use vdisc_qml_bridge::{ApplianceRuntime, Command};
#[path = "../../vdisc-appliance/tests/support/audio_backend.rs"]
mod audio;
use audio::AudioBackend;
fn new() -> ApplianceRuntime<AudioBackend> {
    ApplianceRuntime::with_backend(AudioBackend::default())
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/vdisc")
        .join(name)
}

/// Wait for the asynchronous validator; do not manufacture a verification result.
fn await_validation(runtime: &mut ApplianceRuntime<AudioBackend>) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(result) = runtime.poll_insertion() {
            return result;
        }
        assert!(Instant::now() < deadline, "disc validation timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn initial_projection_has_stable_states_and_empty_lcd_facts() {
    let snapshot = new().snapshot();
    assert_eq!(snapshot.lid_state, "Closed");
    assert_eq!(snapshot.disc_state, "Absent");
    assert_eq!(snapshot.transport_state, "Stopped");
    assert_eq!(snapshot.play_mode, "Normal");
    assert_eq!(snapshot.volume, 0.5);
    assert_eq!(snapshot.application_gain, 0.5);
    assert_eq!(snapshot.machine_error, "");
    assert_eq!(snapshot.lcd_track_number, -1);
    assert_eq!(snapshot.lcd_elapsed_ms, -1);
    assert_eq!(snapshot.lcd_total_tracks, -1);
    assert_eq!(snapshot.lcd_total_ms, -1);
    assert_eq!(snapshot.lcd_playback_status, "Stopped");
    assert_eq!(snapshot.lcd_message, "");
    assert!(!snapshot.lcd_feedback_active);
}

#[test]
fn projection_is_a_value_not_a_second_machine() {
    let mut runtime = new();
    let before = runtime.snapshot();
    runtime.execute(Command::Open).unwrap();
    let after = runtime.snapshot();
    assert_eq!(before.lid_state, "Closed");
    assert_eq!(after.lid_state, "Opening");
}

#[test]
fn volume_hold_and_avls_project_from_controller() {
    let mut runtime = new();
    runtime.execute(Command::SetVolume(0.8)).unwrap();
    assert_eq!(runtime.snapshot().volume, 0.8);
    assert_eq!(runtime.snapshot().application_gain, 0.8);
    runtime.execute(Command::SetHold(true)).unwrap();
    let snapshot = runtime.snapshot();
    assert!(snapshot.hold_enabled);
    assert!(snapshot.lcd_hold);
    runtime.execute(Command::SetHold(false)).unwrap();
    runtime.execute(Command::ToggleAvls).unwrap();
    let snapshot = runtime.snapshot();
    assert!(snapshot.avls_enabled);
    assert!(snapshot.lcd_avls);
    assert_eq!(snapshot.volume, 0.75);
    assert_eq!(snapshot.application_gain, 0.75);
}

#[test]
fn real_disc_facts_enter_lcd_only_after_core_validation() {
    let mut runtime = new();
    runtime.execute(Command::Open).unwrap();
    runtime.execute(Command::LidOpened).unwrap();
    runtime
        .execute(Command::Insert(fixture("valid-v1.vdisc")))
        .unwrap();
    // Insertion only starts the worker; metadata is not yet published.
    let snapshot = runtime.snapshot();
    assert_eq!(snapshot.disc_state, "Inserting");
    assert_eq!(snapshot.lcd_track_number, -1);
    assert_eq!(snapshot.lcd_total_tracks, -1);
    // Physical seating acknowledgement may arrive before verification finishes.
    runtime.execute(Command::DiscInserted).unwrap();
    assert_eq!(runtime.snapshot().disc_state, "Inserting");
    await_validation(&mut runtime).unwrap();
    let verified = runtime.snapshot();
    assert_eq!(verified.disc_state, "Seated");
    assert_eq!(verified.lcd_track_number, 1);
    assert_eq!(verified.lcd_total_tracks, 1);
}

#[test]
fn rejected_disc_projects_machine_error_not_raw_backend_diagnostic() {
    let mut runtime = new();
    runtime.execute(Command::Open).unwrap();
    runtime.execute(Command::LidOpened).unwrap();
    // Starting verification succeeds; rejection is delivered asynchronously.
    runtime
        .execute(Command::Insert(fixture("malformed-manifest.vdisc")))
        .unwrap();
    assert_eq!(runtime.snapshot().disc_state, "Inserting");
    assert!(await_validation(&mut runtime).is_err());
    let snapshot = runtime.snapshot();
    assert_eq!(snapshot.disc_state, "Absent");
    assert_ne!(snapshot.machine_error, "");
    assert_eq!(snapshot.lcd_message, snapshot.machine_error);
    assert_eq!(snapshot.lcd_total_tracks, -1);
}
