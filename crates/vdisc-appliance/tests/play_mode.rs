use vdisc_appliance::{
    De200Controller, PlayMode, PlayModeAction, PlayModeTransitionErrorKind, TrackCompletionIntent,
    Volume,
};

fn controller() -> De200Controller {
    De200Controller::new(Volume::new(0.5).expect("test volume should be valid"))
}

fn playing_controller() -> De200Controller {
    let mut controller = controller();
    controller.request_lid_open().unwrap();
    controller.notify_lid_opened().unwrap();
    controller.request_disc_insert().unwrap();
    controller.notify_disc_validation_accepted().unwrap();
    controller.notify_disc_seated().unwrap();
    controller.request_lid_close().unwrap();
    controller.notify_lid_closed().unwrap();
    controller.request_play().unwrap();
    controller
}

fn set_mode(controller: &mut De200Controller, target: PlayMode) {
    while controller.play_mode() != target {
        controller.request_cycle_play_mode().unwrap();
    }
}

#[test]
fn short_menu_cycles_all_five_modes_and_wraps_to_normal() {
    let mut controller = playing_controller();

    let observed = [
        controller.request_cycle_play_mode().unwrap(),
        controller.request_cycle_play_mode().unwrap(),
        controller.request_cycle_play_mode().unwrap(),
        controller.request_cycle_play_mode().unwrap(),
        controller.request_cycle_play_mode().unwrap(),
    ];

    assert_eq!(
        observed,
        [
            PlayMode::RepeatAll,
            PlayMode::Single,
            PlayMode::RepeatSingle,
            PlayMode::RepeatShuffle,
            PlayMode::Normal,
        ]
    );
}

#[test]
fn play_mode_cycle_requires_active_playback() {
    let mut controller = controller();

    let error = controller.request_cycle_play_mode().unwrap_err();

    assert_eq!(error.action(), PlayModeAction::Cycle);
    assert_eq!(error.play_mode(), PlayMode::Normal);
    assert_eq!(
        error.kind(),
        PlayModeTransitionErrorKind::InvalidTransportState
    );
    assert_eq!(controller.play_mode(), PlayMode::Normal);
}

#[test]
fn normal_mode_advances_nonfinal_track_and_stops_at_disc_end() {
    let controller = playing_controller();

    assert_eq!(
        controller.track_completion_intent(false).unwrap(),
        TrackCompletionIntent::Advance
    );
    assert_eq!(
        controller.track_completion_intent(true).unwrap(),
        TrackCompletionIntent::StopAtDiscEnd
    );
}

#[test]
fn repeat_all_advances_then_restarts_disc_after_last_track() {
    let mut controller = playing_controller();
    set_mode(&mut controller, PlayMode::RepeatAll);

    assert_eq!(
        controller.track_completion_intent(false).unwrap(),
        TrackCompletionIntent::Advance
    );
    assert_eq!(
        controller.track_completion_intent(true).unwrap(),
        TrackCompletionIntent::RestartDisc
    );
}

#[test]
fn single_mode_stops_after_selected_track_regardless_of_disc_position() {
    let mut controller = playing_controller();
    set_mode(&mut controller, PlayMode::Single);

    assert_eq!(
        controller.track_completion_intent(false).unwrap(),
        TrackCompletionIntent::StopAfterCurrent
    );
    assert_eq!(
        controller.track_completion_intent(true).unwrap(),
        TrackCompletionIntent::StopAfterCurrent
    );
}

#[test]
fn repeat_single_replays_current_track() {
    let mut controller = playing_controller();
    set_mode(&mut controller, PlayMode::RepeatSingle);

    assert_eq!(
        controller.track_completion_intent(false).unwrap(),
        TrackCompletionIntent::ReplayCurrent
    );
}

#[test]
fn repeat_shuffle_requests_external_target_without_embedded_rng() {
    let mut controller = playing_controller();
    set_mode(&mut controller, PlayMode::RepeatShuffle);

    for is_last_track in [false, true] {
        assert_eq!(
            controller.track_completion_intent(is_last_track).unwrap(),
            TrackCompletionIntent::ChooseShuffleTrack
        );
    }
}

#[test]
fn completion_policy_requires_playing_transport() {
    let controller = controller();

    let error = controller.track_completion_intent(false).unwrap_err();

    assert_eq!(error.action(), PlayModeAction::TrackCompleted);
    assert_eq!(
        error.kind(),
        PlayModeTransitionErrorKind::InvalidTransportState
    );
}

#[test]
fn policy_queries_do_not_mutate_play_mode_or_transport() {
    let mut controller = playing_controller();
    set_mode(&mut controller, PlayMode::RepeatAll);
    let before_mode = controller.play_mode();
    let before_transport = controller.transport_state();

    let _ = controller.track_completion_intent(true).unwrap();

    assert_eq!(controller.play_mode(), before_mode);
    assert_eq!(controller.transport_state(), before_transport);
}
