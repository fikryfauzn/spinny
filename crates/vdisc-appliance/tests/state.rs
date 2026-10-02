use vdisc_appliance::{
    De200Controller, DiscState, LidState, PlayMode, PlaybackPosition, TransportState, Volume,
    VolumeError,
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
