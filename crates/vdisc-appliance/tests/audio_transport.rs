#![cfg(target_os = "linux")]
#[path = "support/audio_backend.rs"]
mod audio;
use audio::{AudioBackend, Operation};
use vdisc_appliance::{ApplianceErrorState, ScanDirection};
use vdisc_appliance::{
    AudioPlayerBridge, De200Controller, DiscState, PlaybackPosition, TransportState, Volume,
};
use vdisc_core::PlaybackError;
use vdisc_core::PlayerState;

fn seated() -> (
    De200Controller,
    AudioPlayerBridge<AudioBackend>,
    AudioBackend,
) {
    let backend = AudioBackend::default();
    let mut bridge = AudioPlayerBridge::with_backend(backend.clone());
    let mut controller = De200Controller::new(Volume::new(0.5).unwrap());
    controller.request_lid_open().unwrap();
    controller.notify_lid_opened().unwrap();
    controller.request_disc_insert().unwrap();
    bridge
        .validate_inserting_disc(
            &mut controller,
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/vdisc/transport-two-track.vdisc"
            ),
        )
        .unwrap();
    controller.notify_disc_seated().unwrap();
    controller.request_lid_close().unwrap();
    controller.notify_lid_closed().unwrap();
    (controller, bridge, backend)
}

#[test]
fn lazy_play_sets_gain_before_sound_and_rejected_play_has_no_effect() {
    let (mut c, mut b, backend) = seated();
    assert!(backend.operations().is_empty());
    b.play(&mut c).unwrap();
    assert_eq!(
        backend.operations(),
        [Operation::Open(0, 0), Operation::Gain(0.5), Operation::Play]
    );
    let before = backend.operations();
    assert!(b.play(&mut c).is_err());
    assert_eq!(backend.operations(), before);
    assert_eq!(b.backend_state(), PlayerState::Playing);
}

#[test]
fn pause_resume_uses_same_session_stop_captures_live_position() {
    let (mut c, mut b, backend) = seated();
    b.play(&mut c).unwrap();
    backend.current().lock().unwrap().position = 7;
    b.pause(&mut c).unwrap();
    assert_eq!(b.position().as_millis(), 7);
    assert_eq!(c.transport_state(), TransportState::Paused);
    assert!(b.play(&mut c).is_err());
    b.pause(&mut c).unwrap();
    assert_eq!(backend.0.lock().unwrap().sessions.len(), 1);
    b.stop(&mut c).unwrap();
    assert_eq!(c.resume_position(), Some(PlaybackPosition::from_millis(7)));
    assert_eq!(b.position().as_millis(), 0);
    assert_eq!(backend.0.lock().unwrap().active, 0);
    b.play(&mut c).unwrap();
    assert!(backend.operations().ends_with(&[
        Operation::Open(0, 7),
        Operation::Gain(0.5),
        Operation::Play
    ]));
}

#[test]
fn disc_is_ejected_only_on_removal_completion() {
    let (mut c, mut b, backend) = seated();
    assert!(b.complete_disc_removal(&mut c).is_err());
    c.request_lid_open().unwrap();
    c.notify_lid_opened().unwrap();
    c.request_disc_remove().unwrap();
    assert_eq!(b.backend_state(), PlayerState::Stopped);
    b.complete_disc_removal(&mut c).unwrap();
    assert_eq!(c.disc_state(), DiscState::Absent);
    assert_eq!(b.backend_state(), PlayerState::Empty);
    assert!(backend.operations().is_empty());
}

#[test]
fn navigation_preserves_each_transport_and_rejects_boundaries_without_stream_effects() {
    for state in [
        TransportState::Stopped,
        TransportState::Playing,
        TransportState::Paused,
    ] {
        let (mut c, mut b, backend) = seated();
        if state != TransportState::Stopped {
            b.play(&mut c).unwrap();
        }
        if state == TransportState::Paused {
            b.pause(&mut c).unwrap();
        }
        b.next(&mut c).unwrap();
        assert_eq!(b.current_track_index(), Some(1));
        assert_eq!(c.transport_state(), state);
        let before = backend.operations();
        assert!(b.next(&mut c).is_err());
        assert_eq!(backend.operations(), before);
        b.previous(&mut c).unwrap();
        assert_eq!(b.current_track_index(), Some(0));
        let before = backend.operations();
        assert!(b.previous(&mut c).is_err());
        assert_eq!(backend.operations(), before);
        assert_eq!(c.error_state(), None);
        assert!(backend.0.lock().unwrap().max_active <= 1);
        if state == TransportState::Stopped {
            assert!(backend.operations().is_empty());
        }
    }
}

#[test]
fn previous_live_or_saved_position_restarts_and_scan_replaces_at_target() {
    let (mut c, mut b, backend) = seated();
    b.play(&mut c).unwrap();
    b.next(&mut c).unwrap();
    backend.current().lock().unwrap().position = 7;
    b.previous(&mut c).unwrap();
    assert_eq!(b.current_track_index(), Some(1));
    assert_eq!(b.position().as_millis(), 0);
    c.request_scan_begin(ScanDirection::Forward).unwrap();
    b.scan_seek(&mut c, PlaybackPosition::from_millis(7))
        .unwrap();
    c.request_scan_end().unwrap();
    assert_eq!(b.position().as_millis(), 7);
    b.stop(&mut c).unwrap();
    let before = backend.operations();
    b.previous(&mut c).unwrap();
    assert_eq!(backend.operations(), before);
    assert_eq!(c.resume_position(), None);
    assert_eq!(b.current_track_index(), Some(1));
    assert_eq!(backend.0.lock().unwrap().max_active, 1);
}

#[test]
fn failures_silence_both_states_clear_resume_and_keep_disc_then_recover() {
    for error in [
        PlaybackError::NoOutputDevice,
        PlaybackError::UnsupportedOutput {
            sample_rate_hz: 44100,
        },
        PlaybackError::Decode("bad packet".into()),
    ] {
        let want = if matches!(error, PlaybackError::Decode(_)) {
            ApplianceErrorState::PlaybackFailure
        } else {
            ApplianceErrorState::AudioOutputFailure
        };
        let (mut c, mut b, backend) = seated();
        backend.0.lock().unwrap().fail_open = Some(error);
        assert!(b.play(&mut c).is_err());
        assert_eq!(c.error_state(), Some(want));
        assert_eq!(c.transport_state(), TransportState::Stopped);
        assert_eq!(b.backend_state(), PlayerState::Stopped);
        assert_eq!(c.resume_position(), None);
        assert_eq!(c.disc_state(), DiscState::Seated);
        b.sync_gain(&mut c).unwrap();
        assert_eq!(c.error_state(), Some(want));
        b.play(&mut c).unwrap();
        assert_eq!(c.error_state(), None);
    }
    for operation in ["open", "gain", "play", "pause"] {
        let (mut c, mut b, backend) = seated();
        b.play(&mut c).unwrap();
        if operation == "open" {
            backend.0.lock().unwrap().fail_open = Some(PlaybackError::Device("gone".into()));
        } else {
            backend.0.lock().unwrap().fail_operation = Some(operation);
        }
        let result = if operation == "pause" {
            b.pause(&mut c)
        } else {
            b.next(&mut c)
        };
        assert!(result.is_err());
        assert_eq!(c.transport_state(), TransportState::Stopped);
        assert_eq!(b.backend_state(), PlayerState::Stopped);
        assert_eq!(backend.0.lock().unwrap().active, 0);
        assert!(backend.0.lock().unwrap().max_active <= 1);
        assert_eq!(c.resume_position(), None);
        assert_eq!(c.disc_state(), DiscState::Seated);
    }
}

#[test]
fn avls_and_hold_update_live_gain_without_reopening_or_resetting_position() {
    let (mut c, mut b, backend) = seated();
    b.play(&mut c).unwrap();
    backend.current().lock().unwrap().position = 7;
    c.request_set_volume(Volume::new(0.8).unwrap());
    b.sync_gain(&mut c).unwrap();
    c.request_toggle_avls().unwrap();
    b.sync_gain(&mut c).unwrap();
    assert_eq!(backend.current().lock().unwrap().gain, 0.75);
    c.request_toggle_avls().unwrap();
    b.sync_gain(&mut c).unwrap();
    assert_eq!(backend.current().lock().unwrap().gain, 0.75);
    c.set_hold_enabled(true);
    c.request_set_volume(Volume::new(0.0).unwrap());
    b.sync_gain(&mut c).unwrap();
    assert!(c.request_toggle_avls().is_err());
    assert_eq!(backend.current().lock().unwrap().gain, 0.0);
    assert_eq!(b.position().as_millis(), 7);
    assert_eq!(backend.0.lock().unwrap().sessions.len(), 1);
    let before = backend.operations();
    assert!(b.pause(&mut c).is_err());
    assert!(b.stop(&mut c).is_err());
    assert!(b.next(&mut c).is_err());
    assert_eq!(backend.operations(), before);
    c.set_hold_enabled(false);
    b.next(&mut c).unwrap();
    assert_eq!(backend.current().lock().unwrap().gain, 0.0);
    drop(b);
    assert_eq!(backend.0.lock().unwrap().active, 0);
}

#[test]
fn desynchronized_controller_is_a_fail_safe_invariant_not_a_user_rejection() {
    let (mut c, mut b, backend) = seated();
    c.request_play().unwrap(); // simulate a broken upstream integration, not real audio
    assert!(b.pause(&mut c).is_err());
    assert_eq!(c.transport_state(), TransportState::Stopped);
    assert_eq!(b.backend_state(), PlayerState::Stopped);
    assert_eq!(c.error_state(), Some(ApplianceErrorState::PlaybackFailure));
    assert_eq!(backend.0.lock().unwrap().active, 0);
}

#[test]
fn stopped_navigation_does_not_claim_recovery_from_output_failure() {
    let (mut c, mut b, backend) = seated();
    backend.0.lock().unwrap().fail_open = Some(PlaybackError::NoOutputDevice);
    assert!(b.play(&mut c).is_err());
    b.next(&mut c).unwrap();
    assert_eq!(
        c.error_state(),
        Some(ApplianceErrorState::AudioOutputFailure)
    );
    assert_eq!(c.transport_state(), TransportState::Stopped);
    b.previous(&mut c).unwrap();
    assert_eq!(
        c.error_state(),
        Some(ApplianceErrorState::AudioOutputFailure)
    );
    b.play(&mut c).unwrap();
    assert_eq!(c.error_state(), None);
}
