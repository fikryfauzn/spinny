use vdisc_appliance::{
    De200Controller, DiscAction, DiscState, DiscTransitionErrorKind, LidAction, LidState, PlayMode,
    PlaybackPosition, TransportState, Volume, VolumeError,
};

fn initial_volume() -> Volume {
    Volume::new(0.5).expect("test volume should be valid")
}

#[test]
fn new_controller_starts_closed_and_empty() {
    let controller = De200Controller::new(initial_volume());

    assert_eq!(controller.lid_state(), LidState::Closed);
    assert_eq!(controller.disc_state(), DiscState::Absent);
}

#[test]
fn new_controller_starts_stopped() {
    let controller = De200Controller::new(initial_volume());

    assert_eq!(controller.transport_state(), TransportState::Stopped);
}

#[test]
fn new_controller_starts_in_normal_play_mode() {
    let controller = De200Controller::new(initial_volume());

    assert_eq!(controller.play_mode(), PlayMode::Normal);
}

#[test]
fn new_controller_starts_with_machine_flags_disabled() {
    let controller = De200Controller::new(initial_volume());

    assert!(!controller.hold_enabled());
    assert!(!controller.avls_enabled());
}

#[test]
fn new_controller_has_no_resume_position_or_error() {
    let controller = De200Controller::new(initial_volume());

    assert_eq!(controller.resume_position(), None);
    assert_eq!(controller.error_state(), None);
}

#[test]
fn new_controller_preserves_supplied_initial_volume() {
    let volume = Volume::new(0.375).expect("test volume should be valid");
    let controller = De200Controller::new(volume);

    assert_eq!(controller.volume(), volume);
    assert_eq!(controller.volume().normalized(), 0.375);
}

#[test]
fn volume_accepts_inclusive_normalized_bounds() {
    assert_eq!(Volume::new(Volume::MIN).unwrap().normalized(), 0.0);
    assert_eq!(Volume::new(Volume::MAX).unwrap().normalized(), 1.0);
}

#[test]
fn volume_rejects_out_of_range_values() {
    assert_eq!(
        Volume::new(-0.001),
        Err(VolumeError::OutOfRange { value: -0.001 })
    );
    assert_eq!(
        Volume::new(1.001),
        Err(VolumeError::OutOfRange { value: 1.001 })
    );
}

#[test]
fn volume_rejects_non_finite_values() {
    assert_eq!(Volume::new(f32::NAN), Err(VolumeError::NotFinite));
    assert_eq!(Volume::new(f32::INFINITY), Err(VolumeError::NotFinite));
    assert_eq!(Volume::new(f32::NEG_INFINITY), Err(VolumeError::NotFinite));
}

#[test]
fn playback_position_round_trips_milliseconds() {
    let position = PlaybackPosition::from_millis(123_456);

    assert_eq!(position.as_millis(), 123_456);
}

#[test]
fn closed_lid_can_begin_opening_but_does_not_finish_implicitly() {
    let mut controller = De200Controller::new(initial_volume());

    controller.request_lid_open().unwrap();

    assert_eq!(controller.lid_state(), LidState::Opening);
}

#[test]
fn opening_completion_commits_open_state() {
    let mut controller = De200Controller::new(initial_volume());

    controller.request_lid_open().unwrap();
    controller.notify_lid_opened().unwrap();

    assert_eq!(controller.lid_state(), LidState::Open);
}

#[test]
fn open_lid_can_begin_closing_but_does_not_finish_implicitly() {
    let mut controller = De200Controller::new(initial_volume());
    controller.request_lid_open().unwrap();
    controller.notify_lid_opened().unwrap();

    controller.request_lid_close().unwrap();

    assert_eq!(controller.lid_state(), LidState::Closing);
}

#[test]
fn closing_completion_commits_closed_state() {
    let mut controller = De200Controller::new(initial_volume());
    controller.request_lid_open().unwrap();
    controller.notify_lid_opened().unwrap();
    controller.request_lid_close().unwrap();

    controller.notify_lid_closed().unwrap();

    assert_eq!(controller.lid_state(), LidState::Closed);
}

#[test]
fn out_of_order_open_completion_is_rejected_without_mutation() {
    let mut controller = De200Controller::new(initial_volume());

    let error = controller.notify_lid_opened().unwrap_err();

    assert_eq!(error.action(), LidAction::Opened);
    assert_eq!(error.state(), LidState::Closed);
    assert_eq!(controller.lid_state(), LidState::Closed);
}

#[test]
fn duplicate_open_request_is_rejected_without_mutation() {
    let mut controller = De200Controller::new(initial_volume());
    controller.request_lid_open().unwrap();

    let error = controller.request_lid_open().unwrap_err();

    assert_eq!(error.action(), LidAction::RequestOpen);
    assert_eq!(error.state(), LidState::Opening);
    assert_eq!(controller.lid_state(), LidState::Opening);
}

#[test]
fn close_request_requires_fully_open_lid() {
    let mut controller = De200Controller::new(initial_volume());

    let error = controller.request_lid_close().unwrap_err();

    assert_eq!(error.action(), LidAction::RequestClose);
    assert_eq!(error.state(), LidState::Closed);
    assert_eq!(controller.lid_state(), LidState::Closed);
}

#[test]
fn out_of_order_close_completion_is_rejected_without_mutation() {
    let mut controller = De200Controller::new(initial_volume());
    controller.request_lid_open().unwrap();
    controller.notify_lid_opened().unwrap();

    let error = controller.notify_lid_closed().unwrap_err();

    assert_eq!(error.action(), LidAction::Closed);
    assert_eq!(error.state(), LidState::Open);
    assert_eq!(controller.lid_state(), LidState::Open);
}

fn open_lid(controller: &mut De200Controller) {
    controller.request_lid_open().unwrap();
    controller.notify_lid_opened().unwrap();
}

fn seat_disc(controller: &mut De200Controller) {
    open_lid(controller);
    controller.request_disc_insert().unwrap();
    controller.notify_disc_validation_accepted().unwrap();
    controller.notify_disc_seated().unwrap();
}

#[test]
fn insert_requires_fully_open_lid() {
    let mut controller = De200Controller::new(initial_volume());

    let error = controller.request_disc_insert().unwrap_err();

    assert_eq!(error.action(), DiscAction::RequestInsert);
    assert_eq!(error.kind(), DiscTransitionErrorKind::LidNotOpen);
    assert_eq!(error.lid_state(), LidState::Closed);
    assert_eq!(error.disc_state(), DiscState::Absent);
    assert_eq!(controller.disc_state(), DiscState::Absent);
}

#[test]
fn open_empty_player_can_begin_insertion_without_implicit_seating() {
    let mut controller = De200Controller::new(initial_volume());
    open_lid(&mut controller);

    controller.request_disc_insert().unwrap();

    assert_eq!(controller.disc_state(), DiscState::Inserting);
}

#[test]
fn seating_requires_explicit_validation_success() {
    let mut controller = De200Controller::new(initial_volume());
    open_lid(&mut controller);
    controller.request_disc_insert().unwrap();

    let error = controller.notify_disc_seated().unwrap_err();

    assert_eq!(error.action(), DiscAction::Seated);
    assert_eq!(error.kind(), DiscTransitionErrorKind::ValidationRequired);
    assert_eq!(controller.disc_state(), DiscState::Inserting);
}

#[test]
fn validated_insertion_can_commit_seated_state() {
    let mut controller = De200Controller::new(initial_volume());
    open_lid(&mut controller);
    controller.request_disc_insert().unwrap();

    controller.notify_disc_validation_accepted().unwrap();
    assert_eq!(controller.disc_state(), DiscState::Inserting);

    controller.notify_disc_seated().unwrap();
    assert_eq!(controller.disc_state(), DiscState::Seated);
}

#[test]
fn rejected_validation_never_enters_seated_state() {
    let mut controller = De200Controller::new(initial_volume());
    open_lid(&mut controller);
    controller.request_disc_insert().unwrap();

    controller.notify_disc_validation_rejected().unwrap();

    assert_eq!(controller.disc_state(), DiscState::Absent);
}

#[test]
fn duplicate_insert_request_is_rejected_without_mutation() {
    let mut controller = De200Controller::new(initial_volume());
    open_lid(&mut controller);
    controller.request_disc_insert().unwrap();

    let error = controller.request_disc_insert().unwrap_err();

    assert_eq!(error.kind(), DiscTransitionErrorKind::InvalidDiscState);
    assert_eq!(error.disc_state(), DiscState::Inserting);
    assert_eq!(controller.disc_state(), DiscState::Inserting);
}

#[test]
fn remove_requires_fully_open_lid_and_seated_disc() {
    let mut controller = De200Controller::new(initial_volume());
    open_lid(&mut controller);

    let error = controller.request_disc_remove().unwrap_err();

    assert_eq!(error.action(), DiscAction::RequestRemove);
    assert_eq!(error.kind(), DiscTransitionErrorKind::InvalidDiscState);
    assert_eq!(error.disc_state(), DiscState::Absent);
    assert_eq!(controller.disc_state(), DiscState::Absent);
}

#[test]
fn seated_disc_can_begin_removal_and_completion_returns_absent() {
    let mut controller = De200Controller::new(initial_volume());
    seat_disc(&mut controller);

    controller.request_disc_remove().unwrap();
    assert_eq!(controller.disc_state(), DiscState::Removing);

    controller.notify_disc_removed().unwrap();
    assert_eq!(controller.disc_state(), DiscState::Absent);
}

#[test]
fn out_of_order_removal_completion_is_rejected_without_mutation() {
    let mut controller = De200Controller::new(initial_volume());
    seat_disc(&mut controller);

    let error = controller.notify_disc_removed().unwrap_err();

    assert_eq!(error.action(), DiscAction::Removed);
    assert_eq!(error.kind(), DiscTransitionErrorKind::InvalidDiscState);
    assert_eq!(controller.disc_state(), DiscState::Seated);
}

#[test]
fn lid_cannot_close_while_disc_is_inserting() {
    let mut controller = De200Controller::new(initial_volume());
    open_lid(&mut controller);
    controller.request_disc_insert().unwrap();

    let error = controller.request_lid_close().unwrap_err();

    assert_eq!(error.action(), LidAction::RequestClose);
    assert_eq!(error.state(), LidState::Open);
    assert_eq!(error.disc_state(), Some(DiscState::Inserting));
    assert_eq!(controller.lid_state(), LidState::Open);
    assert_eq!(controller.disc_state(), DiscState::Inserting);
}

#[test]
fn lid_cannot_close_while_disc_is_removing() {
    let mut controller = De200Controller::new(initial_volume());
    seat_disc(&mut controller);
    controller.request_disc_remove().unwrap();

    let error = controller.request_lid_close().unwrap_err();

    assert_eq!(error.action(), LidAction::RequestClose);
    assert_eq!(error.state(), LidState::Open);
    assert_eq!(error.disc_state(), Some(DiscState::Removing));
    assert_eq!(controller.lid_state(), LidState::Open);
    assert_eq!(controller.disc_state(), DiscState::Removing);
}

#[test]
fn lid_may_close_with_seated_disc() {
    let mut controller = De200Controller::new(initial_volume());
    seat_disc(&mut controller);

    controller.request_lid_close().unwrap();

    assert_eq!(controller.lid_state(), LidState::Closing);
    assert_eq!(controller.disc_state(), DiscState::Seated);
}
