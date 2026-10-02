use vdisc_appliance::{
    AvlsAction, AvlsTransitionErrorKind, D_E200_AVLS_VOLUME_CEILING, De200Controller, LidState,
    PlayMode, TransportState, Volume,
};

fn volume(value: f32) -> Volume {
    Volume::new(value).expect("test volume must be valid")
}

fn controller(value: f32) -> De200Controller {
    De200Controller::new(volume(value))
}

#[test]
fn avls_ceiling_is_explicit_deterministic_vdisc_policy() {
    assert_eq!(D_E200_AVLS_VOLUME_CEILING.normalized(), 0.75);
}

#[test]
fn volume_without_avls_accepts_full_normalized_range() {
    let mut controller = controller(0.25);

    let applied = controller.request_set_volume(volume(1.0));

    assert_eq!(applied, volume(1.0));
    assert_eq!(controller.volume(), volume(1.0));
    assert!(!controller.avls_enabled());
}

#[test]
fn enabling_avls_immediately_clamps_existing_volume_above_ceiling() {
    let mut controller = controller(1.0);

    let enabled = controller.request_toggle_avls().unwrap();

    assert!(enabled);
    assert!(controller.avls_enabled());
    assert_eq!(controller.volume(), D_E200_AVLS_VOLUME_CEILING);
}

#[test]
fn avls_clamps_new_requests_above_ceiling_and_accepts_lower_values() {
    let mut controller = controller(0.4);
    controller.request_toggle_avls().unwrap();

    let clamped = controller.request_set_volume(volume(1.0));
    assert_eq!(clamped, D_E200_AVLS_VOLUME_CEILING);
    assert_eq!(controller.volume(), D_E200_AVLS_VOLUME_CEILING);

    let lower = controller.request_set_volume(volume(0.5));
    assert_eq!(lower, volume(0.5));
    assert_eq!(controller.volume(), volume(0.5));
}

#[test]
fn disabling_avls_restores_range_without_restoring_old_louder_volume() {
    let mut controller = controller(1.0);
    controller.request_toggle_avls().unwrap();
    assert_eq!(controller.volume(), D_E200_AVLS_VOLUME_CEILING);

    let enabled = controller.request_toggle_avls().unwrap();
    assert!(!enabled);
    assert!(!controller.avls_enabled());
    assert_eq!(controller.volume(), D_E200_AVLS_VOLUME_CEILING);

    let applied = controller.request_set_volume(volume(1.0));
    assert_eq!(applied, volume(1.0));
    assert_eq!(controller.volume(), volume(1.0));
}

#[test]
fn hold_blocks_long_menu_avls_toggle_without_mutation() {
    let mut controller = controller(0.5);
    controller.set_hold_enabled(true);

    let error = controller.request_toggle_avls().unwrap_err();

    assert_eq!(error.action(), AvlsAction::Toggle);
    assert!(error.hold_enabled());
    assert_eq!(error.kind(), AvlsTransitionErrorKind::HoldEnabled);
    assert!(!controller.avls_enabled());
    assert_eq!(controller.volume(), volume(0.5));
}

#[test]
fn volume_remains_adjustable_while_hold_is_enabled() {
    let mut controller = controller(0.25);
    controller.set_hold_enabled(true);

    let applied = controller.request_set_volume(volume(0.9));

    assert_eq!(applied, volume(0.9));
    assert_eq!(controller.volume(), volume(0.9));
    assert!(controller.hold_enabled());
}

#[test]
fn avls_and_volume_commands_do_not_mutate_unrelated_appliance_state() {
    let mut controller = controller(0.6);

    assert_eq!(controller.lid_state(), LidState::Closed);
    assert_eq!(controller.transport_state(), TransportState::Stopped);
    assert_eq!(controller.play_mode(), PlayMode::Normal);
    assert_eq!(controller.resume_position(), None);

    controller.request_toggle_avls().unwrap();
    controller.request_set_volume(volume(0.4));

    assert_eq!(controller.lid_state(), LidState::Closed);
    assert_eq!(controller.transport_state(), TransportState::Stopped);
    assert_eq!(controller.play_mode(), PlayMode::Normal);
    assert_eq!(controller.resume_position(), None);
}
