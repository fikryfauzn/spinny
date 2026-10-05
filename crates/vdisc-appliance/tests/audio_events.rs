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
fn eof_during_scan_ends_scan_before_completion_policy() {
    let (mut c, mut b, backend) = playing(PlayMode::Normal, false);
    c.request_scan_begin(vdisc_appliance::ScanDirection::Forward)
        .unwrap();
    backend.queue(Event::TrackFinished);
    b.poll(&mut c).unwrap();
    assert_eq!(b.current_track_index(), Some(1));
    assert_eq!(c.transport_state(), TransportState::Playing);
}
