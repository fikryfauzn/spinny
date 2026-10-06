#[path = "../../vdisc-appliance/tests/support/audio_backend.rs"]
mod audio;
use audio::{AudioBackend, Operation};
use vdisc_appliance::{ScanDirection, TransportState};
use vdisc_core::{PlaybackBackendEvent, PlaybackError};
use vdisc_qml_bridge::{ApplianceRuntime, Command};

fn seated() -> (ApplianceRuntime<AudioBackend>, AudioBackend) {
    let backend = AudioBackend::default();
    let mut runtime = ApplianceRuntime::with_backend(backend.clone());
    for command in [
        Command::Open,
        Command::LidOpened,
        Command::Insert(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/vdisc/audio-runtime-two-track.vdisc"
            )
            .into(),
        ),
        Command::DiscInserted,
        Command::Close,
        Command::LidClosed,
        Command::Play,
    ] {
        runtime.execute(command).unwrap();
    }
    (runtime, backend)
}

#[test]
fn relative_scan_reads_live_position_and_preserves_gain_session_order() {
    let (mut r, b) = seated();
    assert_eq!(r.backend().current_track_duration_ms(), Some(2_000));
    r.execute(Command::SetVolume(0.4)).unwrap();
    b.current().lock().unwrap().position = 123;
    r.execute(Command::ScanBegin(ScanDirection::Forward))
        .unwrap();
    r.execute(Command::ScanRelative(1_600)).unwrap();
    assert_eq!(r.backend().position().as_millis(), 1_723);
    assert_eq!(r.backend().current_track_index(), Some(0));
    assert!(b.operations().ends_with(&[
        Operation::Open(0, 1_723),
        Operation::Gain(0.4),
        Operation::Play
    ]));
    r.execute(Command::ScanRelative(1_600)).unwrap();
    assert_eq!(r.backend().position().as_millis(), 2_000);
    r.execute(Command::ScanEnd).unwrap();
    r.execute(Command::ScanBegin(ScanDirection::Backward))
        .unwrap();
    b.current().lock().unwrap().position = 1_000;
    r.execute(Command::ScanRelative(-1_600)).unwrap();
    assert_eq!(r.backend().position().as_millis(), 0);
    r.execute(Command::ScanStep(0)).unwrap();
    assert_eq!(b.0.lock().unwrap().max_active, 1);
}

#[test]
fn invalid_relative_input_does_not_replace_stream_or_change_position() {
    let (mut r, b) = seated();
    for c in [
        Command::ScanRelative(0),
        Command::ScanRelative(1),
        Command::ScanRelative(-1),
    ] {
        let before = b.operations();
        assert!(r.execute(c).is_err());
        assert_eq!(b.operations(), before);
    }
    r.execute(Command::ScanBegin(ScanDirection::Forward))
        .unwrap();
    let before = b.operations();
    assert!(r.execute(Command::ScanRelative(-1)).is_err());
    assert_eq!(b.operations(), before);
    r.execute(Command::ScanEnd).unwrap();
    r.execute(Command::Pause).unwrap();
    assert!(r.execute(Command::ScanRelative(1)).is_err());
    r.execute(Command::Stop).unwrap();
    assert!(r.execute(Command::ScanRelative(1)).is_err());
}

#[test]
fn hold_feedback_does_not_supply_scan_position() {
    let (mut r, b) = seated();
    r.execute(Command::ScanBegin(ScanDirection::Forward))
        .unwrap();
    r.execute(Command::SetHold(true)).unwrap();
    let before = b.operations();
    assert!(r.execute(Command::ScanRelative(1_600)).is_err());
    assert_eq!(r.snapshot().lcd_message, "Hold");
    assert_eq!(b.operations(), before);
    r.execute(Command::SetHold(false)).unwrap();
    b.current().lock().unwrap().position = 111;
    r.execute(Command::ScanRelative(1_600)).unwrap();
    assert_eq!(r.backend().position().as_millis(), 1_711);
}

#[test]
fn command_time_eof_ends_scan_before_any_new_track_seek() {
    for mode in [0, 1, 3] {
        // Normal, RepeatAll, RepeatSingle
        let (mut r, b) = seated();
        for _ in 0..mode {
            r.execute(Command::MenuShort).unwrap();
        }
        r.execute(Command::ScanBegin(ScanDirection::Forward))
            .unwrap();
        b.queue(PlaybackBackendEvent::TrackFinished);
        assert!(r.execute(Command::ScanRelative(1_600)).is_err());
        assert_eq!(r.controller().transport_state(), TransportState::Playing);
        assert_eq!(r.backend().position().as_millis(), 0);
        assert_eq!(
            r.backend().current_track_index(),
            Some(if mode == 3 { 0 } else { 1 })
        );
        let before = b.operations();
        assert!(r.execute(Command::ScanRelative(1_600)).is_err());
        assert_eq!(b.operations(), before);
    }
    let (mut r, b) = seated();
    r.execute(Command::Next).unwrap();
    r.execute(Command::ScanBegin(ScanDirection::Forward))
        .unwrap();
    b.queue(PlaybackBackendEvent::TrackFinished);
    assert!(r.execute(Command::ScanRelative(1_600)).is_err());
    assert_eq!(r.controller().transport_state(), TransportState::Stopped);
    assert_eq!(b.0.lock().unwrap().active, 0);
}

#[test]
fn command_time_failure_wins_over_hold_and_never_reopens() {
    for (event, want) in [
        (
            PlaybackBackendEvent::DeviceError("gone".into()),
            "AudioOutputFailure",
        ),
        (
            PlaybackBackendEvent::DecodeError("bad".into()),
            "PlaybackFailure",
        ),
    ] {
        let (mut r, b) = seated();
        r.execute(Command::ScanBegin(ScanDirection::Forward))
            .unwrap();
        r.execute(Command::SetHold(true)).unwrap();
        b.queue(event);
        assert!(r.execute(Command::ScanRelative(1_600)).is_err());
        assert_eq!(r.snapshot().machine_error, want);
        assert!(!r.snapshot().lcd_feedback_active);
        assert_eq!(b.0.lock().unwrap().active, 0);
        assert_eq!(b.0.lock().unwrap().sessions.len(), 1);
    }
    let (mut r, b) = seated();
    r.execute(Command::ScanBegin(ScanDirection::Forward))
        .unwrap();
    b.0.lock().unwrap().fail_open = Some(PlaybackError::Device("gone".into()));
    assert!(r.execute(Command::ScanRelative(1_600)).is_err());
    assert_eq!(r.snapshot().machine_error, "AudioOutputFailure");
    assert_eq!(b.0.lock().unwrap().active, 0);
}
