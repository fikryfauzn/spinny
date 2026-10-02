use vdisc_appliance::{
    ApplianceErrorState, De200Controller, DiscFailureClass, DiscState, LcdBackendFacts, LcdMessage,
    PlaybackFailureClass, PlaybackPosition, TransportState, Volume, translate_disc_failure,
    translate_playback_failure,
};

fn volume(value: f32) -> Volume {
    Volume::new(value).unwrap()
}

fn inserting_controller() -> De200Controller {
    let mut controller = De200Controller::new(volume(0.5));
    controller.request_lid_open().unwrap();
    controller.notify_lid_opened().unwrap();
    controller.request_disc_insert().unwrap();
    controller
}

fn playing_controller() -> De200Controller {
    let mut controller = inserting_controller();
    controller.notify_disc_validation_accepted().unwrap();
    controller.notify_disc_seated().unwrap();
    controller.request_lid_close().unwrap();
    controller.notify_lid_closed().unwrap();
    controller.request_play().unwrap();
    controller
}

#[test]
fn disc_failure_classes_translate_to_the_two_disc_machine_states() {
    assert_eq!(
        translate_disc_failure(DiscFailureClass::InvalidContent),
        ApplianceErrorState::InvalidDisc
    );
    assert_eq!(
        translate_disc_failure(DiscFailureClass::ReadFailure),
        ApplianceErrorState::UnreadableDisc
    );
    assert_eq!(
        translate_disc_failure(DiscFailureClass::ResourceFailure),
        ApplianceErrorState::UnreadableDisc
    );
}

#[test]
fn playback_failure_classes_translate_to_playback_or_audio_output_states() {
    for failure in [
        PlaybackFailureClass::TransportFailure,
        PlaybackFailureClass::DecodeFailure,
        PlaybackFailureClass::BackendInvariant,
    ] {
        assert_eq!(
            translate_playback_failure(failure),
            ApplianceErrorState::PlaybackFailure
        );
    }

    for failure in [
        PlaybackFailureClass::NoOutputDevice,
        PlaybackFailureClass::UnsupportedOutput,
        PlaybackFailureClass::DeviceFailure,
    ] {
        assert_eq!(
            translate_playback_failure(failure),
            ApplianceErrorState::AudioOutputFailure
        );
    }
}

#[test]
fn playback_disc_failures_delegate_to_disc_translation() {
    assert_eq!(
        translate_playback_failure(PlaybackFailureClass::Disc(DiscFailureClass::InvalidContent)),
        ApplianceErrorState::InvalidDisc
    );
    assert_eq!(
        translate_playback_failure(PlaybackFailureClass::Disc(DiscFailureClass::ReadFailure)),
        ApplianceErrorState::UnreadableDisc
    );
}

#[test]
fn invalid_disc_validation_failure_rejects_insertion_and_surfaces_invalid_disc() {
    let mut controller = inserting_controller();

    let translated = controller
        .notify_disc_validation_failed(DiscFailureClass::InvalidContent)
        .unwrap();

    assert_eq!(translated, ApplianceErrorState::InvalidDisc);
    assert_eq!(controller.disc_state(), DiscState::Absent);
    assert_eq!(
        controller.error_state(),
        Some(ApplianceErrorState::InvalidDisc)
    );
    assert_eq!(
        controller
            .lcd_snapshot(LcdBackendFacts::default(), None)
            .message(),
        Some(LcdMessage::InvalidDisc)
    );
}

#[test]
fn unreadable_disc_validation_failure_rejects_insertion_and_surfaces_unreadable_disc() {
    for failure in [
        DiscFailureClass::ReadFailure,
        DiscFailureClass::ResourceFailure,
    ] {
        let mut controller = inserting_controller();

        let translated = controller.notify_disc_validation_failed(failure).unwrap();

        assert_eq!(translated, ApplianceErrorState::UnreadableDisc);
        assert_eq!(controller.disc_state(), DiscState::Absent);
        assert_eq!(
            controller.error_state(),
            Some(ApplianceErrorState::UnreadableDisc)
        );
    }
}

#[test]
fn playback_failure_reporting_changes_only_machine_error_state() {
    let mut controller = playing_controller();
    controller.request_cycle_play_mode().unwrap();
    controller.request_toggle_avls().unwrap();
    controller
        .request_stop(PlaybackPosition::from_millis(42_000))
        .unwrap();

    let before_lid = controller.lid_state();
    let before_disc = controller.disc_state();
    let before_transport = controller.transport_state();
    let before_mode = controller.play_mode();
    let before_hold = controller.hold_enabled();
    let before_avls = controller.avls_enabled();
    let before_volume = controller.volume();
    let before_resume = controller.resume_position();

    let translated = controller.report_playback_failure(PlaybackFailureClass::DecodeFailure);

    assert_eq!(translated, ApplianceErrorState::PlaybackFailure);
    assert_eq!(controller.lid_state(), before_lid);
    assert_eq!(controller.disc_state(), before_disc);
    assert_eq!(controller.transport_state(), before_transport);
    assert_eq!(controller.play_mode(), before_mode);
    assert_eq!(controller.hold_enabled(), before_hold);
    assert_eq!(controller.avls_enabled(), before_avls);
    assert_eq!(controller.volume(), before_volume);
    assert_eq!(controller.resume_position(), before_resume);
    assert_eq!(
        controller.error_state(),
        Some(ApplianceErrorState::PlaybackFailure)
    );
}

#[test]
fn audio_output_failure_projects_existing_constrained_lcd_message() {
    let mut controller = playing_controller();

    controller.report_playback_failure(PlaybackFailureClass::NoOutputDevice);

    let snapshot = controller.lcd_snapshot(
        LcdBackendFacts::new(
            Some(1),
            Some(PlaybackPosition::from_millis(1_000)),
            Some(6),
            Some(PlaybackPosition::from_millis(200_000)),
        ),
        None,
    );

    assert_eq!(snapshot.message(), Some(LcdMessage::AudioOutputFailure));
    assert_eq!(snapshot.track_number(), None);
    assert_eq!(controller.transport_state(), TransportState::Playing);
}

#[test]
fn clearing_error_is_explicit_and_restores_normal_lcd_projection() {
    let mut controller = playing_controller();
    let facts = LcdBackendFacts::new(
        Some(3),
        Some(PlaybackPosition::from_millis(9_000)),
        Some(6),
        Some(PlaybackPosition::from_millis(240_000)),
    );

    controller.report_playback_failure(PlaybackFailureClass::DeviceFailure);
    assert_eq!(
        controller.lcd_snapshot(facts, None).message(),
        Some(LcdMessage::AudioOutputFailure)
    );

    controller.clear_error_state();
    let recovered = controller.lcd_snapshot(facts, None);

    assert_eq!(controller.error_state(), None);
    assert_eq!(recovered.message(), None);
    assert_eq!(recovered.track_number(), Some(3));
    assert_eq!(
        recovered.elapsed_time(),
        Some(PlaybackPosition::from_millis(9_000))
    );
}

#[test]
fn failed_validation_translation_is_transactional_when_no_insertion_is_active() {
    let mut controller = De200Controller::new(volume(0.5));

    let result = controller.notify_disc_validation_failed(DiscFailureClass::InvalidContent);

    assert!(result.is_err());
    assert_eq!(controller.disc_state(), DiscState::Absent);
    assert_eq!(controller.error_state(), None);
}
