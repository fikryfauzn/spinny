use vdisc_appliance::{
    De200Controller, DiscState, LidAction, LidState, PlayMode, PlaybackPosition, TransportState,
    Volume, VolumeError,
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
