#![cfg(target_os = "linux")]
#[path = "support/audio_backend.rs"]
mod audio;
use audio::{AudioBackend, Operation};
use vdisc_appliance::{
    ApplianceErrorState, AudioPlayerBridge, De200Controller, DiscState, PlayMode, TransportState,
    Volume,
};
use vdisc_core::{PlaybackBackendEvent as Event, PlaybackError, PlayerState};

fn playing(
    mode: PlayMode,
    one: bool,
) -> (
    De200Controller,
    AudioPlayerBridge<AudioBackend>,
    AudioBackend,
) {
    let backend = AudioBackend::default();
    let mut b = AudioPlayerBridge::with_backend_and_seed(backend.clone(), 42);
    let mut c = De200Controller::new(Volume::new(0.5).unwrap());
    c.request_lid_open().unwrap();
    c.notify_lid_opened().unwrap();
    c.request_disc_insert().unwrap();
    let fixture = if one {
        "valid-v1.vdisc"
    } else {
        "transport-two-track.vdisc"
    };
    b.validate_inserting_disc(
        &mut c,
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/vdisc")
            .join(fixture),
    )
    .unwrap();
    c.notify_disc_seated().unwrap();
    c.request_lid_close().unwrap();
    c.notify_lid_closed().unwrap();
    b.play(&mut c).unwrap();
    while c.play_mode() != mode {
        c.request_cycle_play_mode().unwrap();
    }
    (c, b, backend)
}
#[test]
fn normal_completion_advances_then_stops_resets_and_clears_resume() {
    let (mut c, mut b, backend) = playing(PlayMode::Normal, false);
    backend.queue(Event::TrackFinished);
    b.poll(&mut c).unwrap();
    assert_eq!(b.current_track_index(), Some(1));
    assert_eq!(c.transport_state(), TransportState::Playing);
    assert!(backend.operations().ends_with(&[
        Operation::Open(1, 0),
        Operation::Gain(0.5),
        Operation::Play
    ]));
    backend.queue(Event::TrackFinished);
    b.poll(&mut c).unwrap();
    assert_eq!(b.current_track_index(), Some(0));
    assert_eq!(b.backend_state(), PlayerState::Stopped);
    assert_eq!(c.transport_state(), TransportState::Stopped);
    assert_eq!(c.resume_position(), None);
    assert_eq!(backend.0.lock().unwrap().active, 0);
}
#[test]
fn all_repeat_modes_hold_and_one_track_shuffle_follow_appliance_policy() {
    for (mode, want, stopped) in [
        (PlayMode::RepeatAll, 0, false),
        (PlayMode::Single, 1, true),
        (PlayMode::RepeatSingle, 1, false),
        (PlayMode::RepeatShuffle, 0, false),
    ] {
        let (mut c, mut b, backend) = playing(mode, false);
        b.next(&mut c).unwrap();
        c.set_hold_enabled(true);
        backend.queue(Event::TrackFinished);
        b.poll(&mut c).unwrap();
        assert_eq!(b.current_track_index(), Some(want));
        assert_eq!(
            c.transport_state(),
            if stopped {
                TransportState::Stopped
            } else {
                TransportState::Playing
            }
        );
        assert_eq!(c.resume_position(), None);
        if !stopped {
            assert!(backend.operations().ends_with(&[
                Operation::Open(want, 0),
                Operation::Gain(0.5),
                Operation::Play
            ]));
        }
        assert_eq!(backend.0.lock().unwrap().max_active, 1);
    }
    let (mut c, mut b, backend) = playing(PlayMode::RepeatShuffle, true);
    backend.queue(Event::TrackFinished);
    b.poll(&mut c).unwrap();
    assert_eq!(b.current_track_index(), Some(0));
    assert_eq!(c.transport_state(), TransportState::Playing);
}
#[test]
fn paused_eof_waits_for_resume_and_current_failures_still_stop() {
    let (mut c, mut b, backend) = playing(PlayMode::Normal, false);
    b.pause(&mut c).unwrap();
    backend.queue(Event::TrackFinished);
    b.poll(&mut c).unwrap();
    assert_eq!(b.current_track_index(), Some(0));
    assert_eq!(c.transport_state(), TransportState::Paused);
    b.pause(&mut c).unwrap();
    b.poll(&mut c).unwrap();
    assert_eq!(b.current_track_index(), Some(1));
    b.poll(&mut c).unwrap();
    assert_eq!(backend.0.lock().unwrap().sessions.len(), 2);
    for event in [
        Event::DeviceError("gone".into()),
        Event::DecodeError("packet".into()),
    ] {
        let want = if matches!(event, Event::DeviceError(_)) {
            ApplianceErrorState::AudioOutputFailure
        } else {
            ApplianceErrorState::PlaybackFailure
        };
        let (mut c, mut b, backend) = playing(PlayMode::Normal, false);
        b.pause(&mut c).unwrap();
        backend.queue(Event::TrackFinished);
        backend.queue(event);
        assert!(b.poll(&mut c).is_err());
        assert_eq!(c.error_state(), Some(want));
        assert_eq!(c.transport_state(), TransportState::Stopped);
        assert_eq!(backend.0.lock().unwrap().active, 0);
        b.poll(&mut c).unwrap();
        assert_eq!(c.error_state(), Some(want));
    }
}
#[test]
fn eof_is_reconciled_before_commands_and_retired_events_cannot_touch_new_session() {
    let (mut c, mut b, backend) = playing(PlayMode::Normal, false);
    let retired = backend.current();
    backend.queue(Event::TrackFinished);
    b.stop(&mut c).unwrap();
    assert_eq!(b.current_track_index(), Some(1));
    assert_eq!(c.transport_state(), TransportState::Stopped);
    b.play(&mut c).unwrap();
    retired
        .lock()
        .unwrap()
        .events
        .extend([Event::TrackFinished, Event::DeviceError("stale".into())]);
    b.poll(&mut c).unwrap();
    assert_eq!(c.transport_state(), TransportState::Playing);
    assert_eq!(c.error_state(), None);
    let (mut c, mut b, backend) = playing(PlayMode::Normal, false);
    backend.queue(Event::TrackFinished);
    assert!(b.next(&mut c).is_err()); // already advanced to final track; no second transition
    assert_eq!(b.current_track_index(), Some(1));
    assert_eq!(backend.0.lock().unwrap().sessions.len(), 2);
}
#[test]
fn eof_followed_by_failure_and_failed_replacement_leave_no_orphan() {
    for failure_in_queue in [true, false] {
        let (mut c, mut b, backend) = playing(PlayMode::Normal, false);
        backend.queue(Event::TrackFinished);
        if failure_in_queue {
            backend.queue(Event::DecodeError("after eof".into()));
        } else {
            backend.0.lock().unwrap().fail_open = Some(PlaybackError::NoOutputDevice);
        }
        assert!(b.poll(&mut c).is_err());
        assert_eq!(c.disc_state(), DiscState::Seated);
        assert_eq!(c.resume_position(), None);
        assert_eq!(b.backend_state(), PlayerState::Stopped);
        assert_eq!(c.transport_state(), TransportState::Stopped);
        assert_eq!(backend.0.lock().unwrap().active, 0);
    }
}
#[test]
fn poll_budgets_sixteen_events_and_one_transition() {
    let (mut c, mut b, backend) = playing(PlayMode::RepeatSingle, false);
    b.pause(&mut c).unwrap();
    for _ in 0..40 {
        backend.queue(Event::TrackFinished);
    }
    let before = backend.0.lock().unwrap().event_polls;
    b.poll(&mut c).unwrap();
    assert_eq!(backend.0.lock().unwrap().event_polls - before, 16);
    assert_eq!(backend.current().lock().unwrap().events.len(), 24);
    b.pause(&mut c).unwrap();
    let before = backend.0.lock().unwrap().sessions.len();
    b.poll(&mut c).unwrap();
    assert_eq!(backend.0.lock().unwrap().sessions.len(), before + 1);
}

#[test]
fn late_integrity_failure_reports_once_and_disc_swap_recovers() {
    use std::{
        fs,
        io::Read,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };
    use vdisc_core::BurnedDisc;

    let input = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/vdisc/transport-two-track.vdisc");
    let strict = BurnedDisc::open(&input).unwrap();
    let name = strict.tracks()[1].path.as_bytes();
    let mut source_track = Vec::new();
    strict
        .track_payload(1)
        .unwrap()
        .unwrap()
        .read_to_end(&mut source_track)
        .unwrap();
    let mut bytes = fs::read(&input).unwrap();
    let mut matches = Vec::new();
    for (header, signature) in bytes.windows(4).enumerate() {
        if signature != b"PK\x03\x04" || header + 30 > bytes.len() {
            continue;
        }
        let name_len = u16::from_le_bytes([bytes[header + 26], bytes[header + 27]]) as usize;
        let extra_len = u16::from_le_bytes([bytes[header + 28], bytes[header + 29]]) as usize;
        let name_start = header + 30;
        let data_start = name_start + name_len + extra_len;
        if bytes.get(name_start..name_start + name_len) == Some(name)
            && bytes.get(data_start..data_start + source_track.len())
                == Some(source_track.as_slice())
        {
            matches.push(data_start);
        }
    }
    assert_eq!(matches.len(), 1);
    bytes[matches[0] + source_track.len() / 2] ^= 0x80;
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "vdisc-late-failure-{}-{unique}.vdisc",
        std::process::id()
    ));
    fs::write(&path, bytes).unwrap();
    struct RemoveFile(std::path::PathBuf);
    impl Drop for RemoveFile {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }
    let _cleanup = RemoveFile(path.clone());

    let disc = BurnedDisc::open_for_playback(&path).unwrap();
    let backend = AudioBackend::default();
    let mut bridge = AudioPlayerBridge::with_backend(backend);
    let mut controller = De200Controller::new(Volume::new(0.5).unwrap());
    controller.request_lid_open().unwrap();
    controller.notify_lid_opened().unwrap();
    controller.request_disc_insert().unwrap();
    bridge.accept_validated_disc(&mut controller, disc).unwrap();
    controller.notify_disc_seated().unwrap();
    controller.request_lid_close().unwrap();
    controller.notify_lid_closed().unwrap();

    // Tiny fixtures may finish verification before PLAY. Either the PLAY
    // pre-poll or the periodic pump must surface the same late corruption.
    let start = Instant::now();
    let mut failed = bridge.play(&mut controller).is_err();
    while !failed && start.elapsed() < Duration::from_secs(5) {
        failed = bridge.poll(&mut controller).is_err();
        if !failed {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    assert!(failed, "background integrity failure was not reported");
    assert_eq!(
        controller.error_state(),
        Some(ApplianceErrorState::InvalidDisc)
    );
    assert_eq!(controller.transport_state(), TransportState::Stopped);
    bridge.poll(&mut controller).unwrap(); // no repeated error storm

    controller.request_lid_open().unwrap();
    controller.notify_lid_opened().unwrap();
    controller.request_disc_remove().unwrap();
    bridge.complete_disc_removal(&mut controller).unwrap();
    controller.request_disc_insert().unwrap();
    bridge
        .validate_inserting_disc(&mut controller, &input)
        .unwrap();
    controller.notify_disc_seated().unwrap();
    controller.request_lid_close().unwrap();
    controller.notify_lid_closed().unwrap();
    bridge.play(&mut controller).unwrap();
    assert_eq!(controller.error_state(), None);
    assert_eq!(controller.transport_state(), TransportState::Playing);
}

#[test]
fn eof_during_scan_ends_scan_before_completion_policy() {
    let (mut c, mut b, backend) = playing(PlayMode::Normal, false);
    c.request_scan_begin(vdisc_appliance::ScanDirection::Forward)
        .unwrap();
    backend.queue(Event::TrackFinished);
    b.poll(&mut c).unwrap();
    assert_eq!(b.current_track_index(), Some(1));
    assert_eq!(c.transport_state(), TransportState::Playing);
}
