use vdisc_appliance::{
    De200Controller, DiscState, LidState, PlaybackPosition, TransportAction, TransportState,
    TransportTransitionErrorKind, Volume,
};

fn controller() -> De200Controller {
    De200Controller::new(Volume::new(0.5).expect("test volume should be valid"))
}

fn seat_and_close_disc(controller: &mut De200Controller) {
    controller.request_lid_open().unwrap();
    controller.notify_lid_opened().unwrap();
    controller.request_disc_insert().unwrap();
    controller.notify_disc_validation_accepted().unwrap();
    controller.notify_disc_seated().unwrap();
    controller.request_lid_close().unwrap();
    controller.notify_lid_closed().unwrap();

    assert_eq!(controller.lid_state(), LidState::Closed);
    assert_eq!(controller.disc_state(), DiscState::Seated);
}

#[test]
fn transport_play_requires_closed_lid_and_seated_disc() {
    let mut controller = controller();

    let no_disc = controller.request_play().unwrap_err();
    assert_eq!(no_disc.action(), TransportAction::Play);
    assert_eq!(no_disc.kind(), TransportTransitionErrorKind::DiscNotSeated);
    assert_eq!(controller.transport_state(), TransportState::Stopped);

    controller.request_lid_open().unwrap();
    controller.notify_lid_opened().unwrap();
    controller.request_disc_insert().unwrap();
    controller.notify_disc_validation_accepted().unwrap();
    controller.notify_disc_seated().unwrap();

    let open_lid = controller.request_play().unwrap_err();
    assert_eq!(open_lid.action(), TransportAction::Play);
    assert_eq!(open_lid.kind(), TransportTransitionErrorKind::LidNotClosed);
    assert_eq!(controller.transport_state(), TransportState::Stopped);
}

#[test]
fn transport_play_moves_stopped_to_playing_without_changing_mechanics() {
    let mut controller = controller();
    seat_and_close_disc(&mut controller);

    controller.request_play().unwrap();

    assert_eq!(controller.transport_state(), TransportState::Playing);
    assert_eq!(controller.lid_state(), LidState::Closed);
    assert_eq!(controller.disc_state(), DiscState::Seated);
}

#[test]
fn transport_duplicate_play_is_rejected_without_mutation() {
    let mut controller = controller();
    seat_and_close_disc(&mut controller);
    controller.request_play().unwrap();

    let error = controller.request_play().unwrap_err();

    assert_eq!(error.action(), TransportAction::Play);
    assert_eq!(
        error.kind(),
        TransportTransitionErrorKind::InvalidTransportState
    );
    assert_eq!(error.transport_state(), TransportState::Playing);
    assert_eq!(controller.transport_state(), TransportState::Playing);
}

#[test]
fn transport_pause_toggles_playing_and_paused() {
    let mut controller = controller();
    seat_and_close_disc(&mut controller);
    controller.request_play().unwrap();

    controller.request_pause().unwrap();
    assert_eq!(controller.transport_state(), TransportState::Paused);

    controller.request_pause().unwrap();
    assert_eq!(controller.transport_state(), TransportState::Playing);
}

#[test]
fn transport_pause_from_stopped_is_controlled_rejection() {
    let mut controller = controller();
    seat_and_close_disc(&mut controller);

    let error = controller.request_pause().unwrap_err();

    assert_eq!(error.action(), TransportAction::Pause);
    assert_eq!(
        error.kind(),
        TransportTransitionErrorKind::InvalidTransportState
    );
    assert_eq!(controller.transport_state(), TransportState::Stopped);
}

#[test]
fn transport_stop_from_playing_captures_resume_position_before_stopping() {
    let mut controller = controller();
    seat_and_close_disc(&mut controller);
    controller.request_play().unwrap();
    let position = PlaybackPosition::from_millis(91_250);

    controller.request_stop(position).unwrap();

    assert_eq!(controller.transport_state(), TransportState::Stopped);
    assert_eq!(controller.resume_position(), Some(position));
    assert_eq!(controller.lid_state(), LidState::Closed);
    assert_eq!(controller.disc_state(), DiscState::Seated);
}

#[test]
fn transport_stop_from_paused_captures_resume_position() {
    let mut controller = controller();
    seat_and_close_disc(&mut controller);
    controller.request_play().unwrap();
    controller.request_pause().unwrap();
    let position = PlaybackPosition::from_millis(17_000);

    controller.request_stop(position).unwrap();

    assert_eq!(controller.transport_state(), TransportState::Stopped);
    assert_eq!(controller.resume_position(), Some(position));
}

#[test]
fn transport_stop_from_stopped_is_rejected_without_overwriting_resume_memory() {
    let mut controller = controller();
    seat_and_close_disc(&mut controller);
    controller.request_play().unwrap();
    let remembered = PlaybackPosition::from_millis(8_000);
    controller.request_stop(remembered).unwrap();

    let error = controller
        .request_stop(PlaybackPosition::from_millis(99_999))
        .unwrap_err();

    assert_eq!(error.action(), TransportAction::Stop);
    assert_eq!(
        error.kind(),
        TransportTransitionErrorKind::InvalidTransportState
    );
    assert_eq!(controller.transport_state(), TransportState::Stopped);
    assert_eq!(controller.resume_position(), Some(remembered));
}

#[test]
fn transport_stop_does_not_open_or_eject_the_disc() {
    let mut controller = controller();
    seat_and_close_disc(&mut controller);
    controller.request_play().unwrap();

    controller
        .request_stop(PlaybackPosition::from_millis(1_234))
        .unwrap();

    assert_eq!(controller.lid_state(), LidState::Closed);
    assert_eq!(controller.disc_state(), DiscState::Seated);
}
