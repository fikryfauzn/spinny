use vdisc_appliance::{
    De200Controller, LcdBackendFacts, LcdMessage, LcdPlaybackStatus, LcdTransientMessage, PlayMode,
    PlaybackPosition, ScanDirection, Volume,
};

fn volume(value: f32) -> Volume {
    Volume::new(value).unwrap()
}

fn ready_controller() -> De200Controller {
    let mut controller = De200Controller::new(volume(0.5));
    controller.request_lid_open().unwrap();
    controller.notify_lid_opened().unwrap();
    controller.request_disc_insert().unwrap();
    controller.notify_disc_validation_accepted().unwrap();
    controller.notify_disc_seated().unwrap();
    controller.request_lid_close().unwrap();
    controller.notify_lid_closed().unwrap();
    controller
}

fn playing_controller() -> De200Controller {
    let mut controller = ready_controller();
    controller.request_play().unwrap();
    controller
}

fn backend_facts() -> LcdBackendFacts {
    LcdBackendFacts::new(
        Some(2),
        Some(PlaybackPosition::from_millis(12_345)),
        Some(6),
        Some(PlaybackPosition::from_millis(321_000)),
    )
}

#[test]
fn initial_snapshot_is_stopped_with_normal_mode_and_no_indicators() {
    let controller = De200Controller::new(volume(0.5));
    let snapshot = controller.lcd_snapshot(LcdBackendFacts::default(), None);

    assert_eq!(snapshot.track_number(), None);
    assert_eq!(snapshot.elapsed_time(), None);
    assert_eq!(snapshot.total_tracks(), None);
    assert_eq!(snapshot.total_time(), None);
    assert_eq!(snapshot.play_mode(), PlayMode::Normal);
    assert!(!snapshot.hold_indicator());
    assert!(!snapshot.avls_indicator());
    assert_eq!(snapshot.playback_status(), LcdPlaybackStatus::Stopped);
    assert_eq!(snapshot.message(), None);
}

#[test]
fn normal_snapshot_projects_backend_track_and_time_facts_without_owning_them() {
    let controller = playing_controller();
    let snapshot = controller.lcd_snapshot(backend_facts(), None);

    assert_eq!(snapshot.track_number(), Some(2));
    assert_eq!(
        snapshot.elapsed_time(),
        Some(PlaybackPosition::from_millis(12_345))
    );
    assert_eq!(snapshot.total_tracks(), Some(6));
    assert_eq!(
        snapshot.total_time(),
        Some(PlaybackPosition::from_millis(321_000))
    );
    assert_eq!(snapshot.playback_status(), LcdPlaybackStatus::Playing);
    assert_eq!(snapshot.message(), None);
}

#[test]
fn snapshot_projects_play_mode_hold_and_avls_as_persistent_indicators() {
    let mut controller = playing_controller();
    controller.request_cycle_play_mode().unwrap();
    controller.request_toggle_avls().unwrap();
    controller.set_hold_enabled(true);

    let snapshot = controller.lcd_snapshot(backend_facts(), None);

    assert_eq!(snapshot.play_mode(), PlayMode::RepeatAll);
    assert!(snapshot.hold_indicator());
    assert!(snapshot.avls_indicator());
    assert_eq!(snapshot.message(), None);
    assert_eq!(snapshot.track_number(), Some(2));
}

#[test]
fn transient_hold_message_overrides_track_time_but_preserves_indicators() {
    let mut controller = playing_controller();
    controller.request_cycle_play_mode().unwrap();
    controller.request_toggle_avls().unwrap();
    controller.set_hold_enabled(true);

    let snapshot = controller.lcd_snapshot(backend_facts(), Some(LcdTransientMessage::Hold));

    assert_eq!(snapshot.message(), Some(LcdMessage::Hold));
    assert_eq!(snapshot.track_number(), None);
    assert_eq!(snapshot.elapsed_time(), None);
    assert_eq!(snapshot.total_tracks(), None);
    assert_eq!(snapshot.total_time(), None);
    assert_eq!(snapshot.play_mode(), PlayMode::RepeatAll);
    assert!(snapshot.hold_indicator());
    assert!(snapshot.avls_indicator());
    assert_eq!(snapshot.playback_status(), LcdPlaybackStatus::Playing);
}

#[test]
fn clearing_transient_feedback_restores_normal_track_time_projection() {
    let mut controller = playing_controller();
    controller.set_hold_enabled(true);

    let held = controller.lcd_snapshot(backend_facts(), Some(LcdTransientMessage::Hold));
    let normal = controller.lcd_snapshot(backend_facts(), None);

    assert_eq!(held.track_number(), None);
    assert_eq!(held.message(), Some(LcdMessage::Hold));
    assert_eq!(normal.track_number(), Some(2));
    assert_eq!(
        normal.elapsed_time(),
        Some(PlaybackPosition::from_millis(12_345))
    );
    assert_eq!(normal.message(), None);
}

#[test]
fn paused_and_seeking_transport_have_explicit_lcd_status() {
    let mut paused = playing_controller();
    paused.request_pause().unwrap();
    assert_eq!(
        paused.lcd_snapshot(backend_facts(), None).playback_status(),
        LcdPlaybackStatus::Paused
    );

    let mut forward = playing_controller();
    forward.request_scan_begin(ScanDirection::Forward).unwrap();
    assert_eq!(
        forward
            .lcd_snapshot(backend_facts(), None)
            .playback_status(),
        LcdPlaybackStatus::SeekingForward
    );

    let mut backward = playing_controller();
    backward
        .request_scan_begin(ScanDirection::Backward)
        .unwrap();
    assert_eq!(
        backward
            .lcd_snapshot(backend_facts(), None)
            .playback_status(),
        LcdPlaybackStatus::SeekingBackward
    );
}

#[test]
fn lcd_projection_is_read_only_with_respect_to_appliance_state() {
    let mut controller = playing_controller();
    controller.request_cycle_play_mode().unwrap();
    controller.request_toggle_avls().unwrap();

    let before_lid = controller.lid_state();
    let before_disc = controller.disc_state();
    let before_transport = controller.transport_state();
    let before_mode = controller.play_mode();
    let before_hold = controller.hold_enabled();
    let before_avls = controller.avls_enabled();
    let before_volume = controller.volume();
    let before_resume = controller.resume_position();
    let before_error = controller.error_state();

    let _ = controller.lcd_snapshot(backend_facts(), Some(LcdTransientMessage::Hold));

    assert_eq!(controller.lid_state(), before_lid);
    assert_eq!(controller.disc_state(), before_disc);
    assert_eq!(controller.transport_state(), before_transport);
    assert_eq!(controller.play_mode(), before_mode);
    assert_eq!(controller.hold_enabled(), before_hold);
    assert_eq!(controller.avls_enabled(), before_avls);
    assert_eq!(controller.volume(), before_volume);
    assert_eq!(controller.resume_position(), before_resume);
    assert_eq!(controller.error_state(), before_error);
}
