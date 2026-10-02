#![cfg(target_os = "linux")]

use std::{fs, path::Path};

use vdisc_appliance::{
    ApplianceErrorState, CoreIntegrationError, CorePlayerBridge, De200Controller, DiscFailureClass,
    DiscState, LcdMessage, PlayMode, PlaybackFailureClass, PlaybackPosition, ScanDirection,
    TrackCompletionIntent, TransportState, Volume, classify_format_error, classify_playback_error,
    classify_player_error,
};
use vdisc_core::{
    PlaybackError, PlayerAction, PlayerBoundary, PlayerError, PlayerState,
    format::{FormatError, FormatErrorKind},
};

fn volume(value: f32) -> Volume {
    Volume::new(value).unwrap()
}

fn fixture() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/vdisc/valid-v1.vdisc")
}

fn seated_runtime() -> (De200Controller, CorePlayerBridge) {
    let mut controller = De200Controller::new(volume(0.5));
    let mut bridge = CorePlayerBridge::new();

    controller.request_lid_open().unwrap();
    controller.notify_lid_opened().unwrap();
    controller.request_disc_insert().unwrap();
    bridge
        .validate_inserting_disc(&mut controller, fixture())
        .unwrap();
    controller.notify_disc_seated().unwrap();
    controller.request_lid_close().unwrap();
    controller.notify_lid_closed().unwrap();

    (controller, bridge)
}

#[test]
fn real_fixture_insertion_projects_core_owned_lcd_facts() {
    let (controller, bridge) = seated_runtime();

    assert_eq!(controller.disc_state(), DiscState::Seated);
    assert_eq!(bridge.backend_state(), PlayerState::Stopped);
    assert!(bridge.track_count() >= 1);
    assert_eq!(bridge.current_track_index(), Some(0));

    let snapshot = controller.lcd_snapshot(bridge.lcd_facts(), None);
    assert_eq!(snapshot.track_number(), Some(1));
    assert_eq!(
        snapshot.total_tracks(),
        u8::try_from(bridge.track_count()).ok()
    );
    assert_eq!(snapshot.message(), None);
}

#[test]
fn stop_resume_adapter_restores_position_before_core_play() {
    let (mut controller, mut bridge) = seated_runtime();

    bridge.play(&mut controller).unwrap();
    controller
        .request_scan_begin(ScanDirection::Forward)
        .unwrap();
    bridge
        .scan_seek(&mut controller, PlaybackPosition::from_millis(1))
        .unwrap();
    controller.request_scan_end().unwrap();

    bridge.stop(&mut controller).unwrap();
    assert_eq!(controller.transport_state(), TransportState::Stopped);
    assert_eq!(
        controller.resume_position(),
        Some(PlaybackPosition::from_millis(1))
    );
    assert_eq!(bridge.backend_state(), PlayerState::Stopped);
    assert_eq!(bridge.position(), PlaybackPosition::from_millis(0));

    bridge.play(&mut controller).unwrap();
    assert_eq!(controller.transport_state(), TransportState::Playing);
    assert_eq!(bridge.backend_state(), PlayerState::Playing);
    assert_eq!(bridge.position(), PlaybackPosition::from_millis(1));
}

#[test]
fn pause_toggle_and_stop_commit_only_after_core_succeeds() {
    let (mut controller, mut bridge) = seated_runtime();
    bridge.play(&mut controller).unwrap();

    bridge.pause(&mut controller).unwrap();
    assert_eq!(controller.transport_state(), TransportState::Paused);
    assert_eq!(bridge.backend_state(), PlayerState::Paused);

    bridge.pause(&mut controller).unwrap();
    assert_eq!(controller.transport_state(), TransportState::Playing);
    assert_eq!(bridge.backend_state(), PlayerState::Playing);

    bridge.stop(&mut controller).unwrap();
    assert_eq!(controller.transport_state(), TransportState::Stopped);
    assert_eq!(bridge.backend_state(), PlayerState::Stopped);
}

#[test]
fn completed_physical_removal_ejects_core_and_clears_resume_session() {
    let (mut controller, mut bridge) = seated_runtime();
    bridge.play(&mut controller).unwrap();
    bridge.stop(&mut controller).unwrap();
    assert!(controller.resume_position().is_some());

    controller.request_lid_open().unwrap();
    controller.notify_lid_opened().unwrap();
    controller.request_disc_remove().unwrap();
    bridge.complete_disc_removal(&mut controller).unwrap();

    assert_eq!(controller.disc_state(), DiscState::Absent);
    assert_eq!(controller.resume_position(), None);
    assert_eq!(bridge.backend_state(), PlayerState::Empty);
}

#[test]
fn invalid_real_bytes_are_rejected_and_surface_invalid_disc() {
    let path = std::env::temp_dir().join(format!(
        "vdisc-objective13-invalid-{}.vdisc",
        std::process::id()
    ));
    fs::write(&path, b"not a vdisc").unwrap();

    let mut controller = De200Controller::new(volume(0.5));
    let mut bridge = CorePlayerBridge::new();
    controller.request_lid_open().unwrap();
    controller.notify_lid_opened().unwrap();
    controller.request_disc_insert().unwrap();

    let error = bridge
        .validate_inserting_disc(&mut controller, &path)
        .unwrap_err();
    fs::remove_file(path).ok();

    assert!(matches!(
        error,
        CoreIntegrationError::Backend(PlayerError::Disc(_))
    ));
    assert_eq!(controller.disc_state(), DiscState::Absent);
    assert_eq!(
        controller.error_state(),
        Some(ApplianceErrorState::InvalidDisc)
    );
    assert_eq!(
        controller.lcd_snapshot(bridge.lcd_facts(), None).message(),
        Some(LcdMessage::InvalidDisc)
    );
}

#[test]
fn single_and_repeat_single_modes_execute_against_real_core_state() {
    let (mut single_controller, mut single_bridge) = seated_runtime();
    single_bridge.play(&mut single_controller).unwrap();
    single_controller.request_cycle_play_mode().unwrap();
    single_controller.request_cycle_play_mode().unwrap();
    assert_eq!(single_controller.play_mode(), PlayMode::Single);

    let intent = single_bridge
        .track_finished(&mut single_controller, None)
        .unwrap();
    assert_eq!(intent, TrackCompletionIntent::StopAfterCurrent);
    assert_eq!(single_controller.transport_state(), TransportState::Stopped);
    assert_eq!(single_bridge.backend_state(), PlayerState::Stopped);
    assert_eq!(single_controller.resume_position(), None);

    let (mut repeat_controller, mut repeat_bridge) = seated_runtime();
    repeat_bridge.play(&mut repeat_controller).unwrap();
    for _ in 0..3 {
        repeat_controller.request_cycle_play_mode().unwrap();
    }
    assert_eq!(repeat_controller.play_mode(), PlayMode::RepeatSingle);

    let intent = repeat_bridge
        .track_finished(&mut repeat_controller, None)
        .unwrap();
    assert_eq!(intent, TrackCompletionIntent::ReplayCurrent);
    assert_eq!(repeat_controller.transport_state(), TransportState::Playing);
    assert_eq!(repeat_bridge.backend_state(), PlayerState::Playing);
    assert_eq!(repeat_bridge.position(), PlaybackPosition::from_millis(0));
}

#[test]
fn repeat_shuffle_requires_runtime_target_without_embedding_rng() {
    let (mut controller, mut bridge) = seated_runtime();
    bridge.play(&mut controller).unwrap();

    for _ in 0..4 {
        controller.request_cycle_play_mode().unwrap();
    }
    assert_eq!(controller.play_mode(), PlayMode::RepeatShuffle);

    assert!(matches!(
        bridge.track_finished(&mut controller, None),
        Err(CoreIntegrationError::MissingShuffleTarget)
    ));

    let current = bridge.current_track_index().unwrap();
    let target = if bridge.track_count() == 1 {
        current
    } else {
        (current + 1) % bridge.track_count()
    };
    let intent = bridge
        .track_finished(&mut controller, Some(target))
        .unwrap();
    assert_eq!(intent, TrackCompletionIntent::ChooseShuffleTrack);
    assert_eq!(controller.transport_state(), TransportState::Playing);
    assert_eq!(bridge.backend_state(), PlayerState::Playing);
    assert_eq!(bridge.current_track_index(), Some(target));
}

#[test]
fn application_gain_is_the_exact_post_avls_machine_volume() {
    let (mut controller, bridge) = seated_runtime();
    controller.request_set_volume(volume(1.0));
    assert_eq!(bridge.application_gain(&controller), 1.0);

    controller.request_toggle_avls().unwrap();
    assert_eq!(bridge.application_gain(&controller), 0.75);
}

#[test]
fn concrete_core_errors_map_into_objective12_taxonomy() {
    let invalid_limit = FormatError {
        kind: FormatErrorKind::ResourceLimit,
        entry: None,
        message: "manifest exceeds byte limit".into(),
    };
    let runtime_resource = FormatError {
        kind: FormatErrorKind::ResourceLimit,
        entry: None,
        message: "allocation failed".into(),
    };
    let io = FormatError {
        kind: FormatErrorKind::Io,
        entry: None,
        message: "read failed".into(),
    };

    assert_eq!(
        classify_format_error(&invalid_limit),
        DiscFailureClass::InvalidContent
    );
    assert_eq!(
        classify_format_error(&runtime_resource),
        DiscFailureClass::ResourceFailure
    );
    assert_eq!(classify_format_error(&io), DiscFailureClass::ReadFailure);

    let boundary = PlayerError::Boundary {
        action: PlayerAction::Next,
        boundary: PlayerBoundary::EndOfDisc,
    };
    assert_eq!(
        classify_player_error(&boundary),
        PlaybackFailureClass::TransportFailure
    );

    assert_eq!(
        classify_playback_error(&PlaybackError::NoOutputDevice),
        PlaybackFailureClass::NoOutputDevice
    );
    assert_eq!(
        classify_playback_error(&PlaybackError::Device("gone".into())),
        PlaybackFailureClass::DeviceFailure
    );
    assert_eq!(
        classify_playback_error(&PlaybackError::Decode("bad frame".into())),
        PlaybackFailureClass::DecodeFailure
    );
    assert_eq!(
        classify_playback_error(&PlaybackError::BackendInvariant("broken")),
        PlaybackFailureClass::BackendInvariant
    );
}

#[test]
fn track_boundary_navigation_is_controlled_not_a_machine_failure() {
    let (mut controller, mut bridge) = seated_runtime();

    for _ in 1..bridge.track_count() {
        bridge.next(&mut controller).unwrap();
    }
    let result = bridge.next(&mut controller);
    assert!(matches!(
        result,
        Err(CoreIntegrationError::Navigation(
            vdisc_appliance::NavigationError::Backend(PlayerError::Boundary { .. })
        ))
    ));
    assert_eq!(controller.error_state(), None);
    assert_eq!(controller.transport_state(), TransportState::Stopped);
    assert_eq!(bridge.backend_state(), PlayerState::Stopped);
}
