use vdisc_appliance::{
    De200Controller, LidState, NavigationError, NavigationPlaybackPort,
    NavigationTransitionErrorKind, PlayMode, PlayModeTransitionErrorKind, PlaybackPosition,
    ResumePlayError, ResumePlaybackPort, ScanDirection, TrackCompletionIntent, TransportState,
    TransportTransitionErrorKind, Volume,
};

#[derive(Debug, Default)]
struct FakeNavigationPort {
    seeks: Vec<PlaybackPosition>,
    next_calls: usize,
    previous_calls: usize,
}

impl NavigationPlaybackPort for FakeNavigationPort {
    type Error = &'static str;

    fn seek(&mut self, position: PlaybackPosition) -> Result<(), Self::Error> {
        self.seeks.push(position);
        Ok(())
    }

    fn next_track(&mut self) -> Result<(), Self::Error> {
        self.next_calls += 1;
        Ok(())
    }

    fn previous_track(&mut self) -> Result<(), Self::Error> {
        self.previous_calls += 1;
        Ok(())
    }
}

#[derive(Debug, Default)]
struct FakeResumePort {
    seeks: Vec<PlaybackPosition>,
    play_calls: usize,
}

impl ResumePlaybackPort for FakeResumePort {
    type Error = &'static str;

    fn seek(&mut self, position: PlaybackPosition) -> Result<(), Self::Error> {
        self.seeks.push(position);
        Ok(())
    }

    fn play(&mut self) -> Result<(), Self::Error> {
        self.play_calls += 1;
        Ok(())
    }
}

fn controller() -> De200Controller {
    De200Controller::new(Volume::new(0.5).expect("test volume should be valid"))
}

fn seat_disc_and_close(controller: &mut De200Controller) {
    controller.request_lid_open().unwrap();
    controller.notify_lid_opened().unwrap();
    controller.request_disc_insert().unwrap();
    controller.notify_disc_validation_accepted().unwrap();
    controller.notify_disc_seated().unwrap();
    controller.request_lid_close().unwrap();
    controller.notify_lid_closed().unwrap();
}

fn assert_navigation_hold(error: NavigationError<&'static str>) {
    match error {
        NavigationError::Transition(error) => {
            assert_eq!(error.kind(), NavigationTransitionErrorKind::HoldEnabled)
        }
        NavigationError::Backend(error) => panic!("unexpected backend error: {error}"),
    }
}

#[test]
fn hold_switch_can_always_enable_and_disable_itself() {
    let mut controller = controller();

    assert!(!controller.hold_enabled());
    controller.set_hold_enabled(true);
    assert!(controller.hold_enabled());

    controller.set_hold_enabled(true);
    assert!(controller.hold_enabled());

    controller.set_hold_enabled(false);
    assert!(!controller.hold_enabled());
}

#[test]
fn hold_blocks_play_before_other_play_preconditions() {
    let mut controller = controller();
    controller.set_hold_enabled(true);

    let error = controller.request_play().unwrap_err();

    assert_eq!(error.kind(), TransportTransitionErrorKind::HoldEnabled);
    assert_eq!(controller.transport_state(), TransportState::Stopped);
    assert_eq!(controller.lid_state(), LidState::Closed);
}

#[test]
fn hold_blocks_resume_aware_play_before_backend_effects() {
    let mut controller = controller();
    seat_disc_and_close(&mut controller);
    controller.request_play().unwrap();
    let resume = PlaybackPosition::from_millis(23_000);
    controller.request_stop(resume).unwrap();
    controller.set_hold_enabled(true);
    let mut port = FakeResumePort::default();

    let error = controller.request_play_with(&mut port).unwrap_err();
    match error {
        ResumePlayError::Transition(error) => {
            assert_eq!(error.kind(), TransportTransitionErrorKind::HoldEnabled)
        }
        ResumePlayError::Backend(error) => panic!("unexpected backend error: {error}"),
    }

    assert!(port.seeks.is_empty());
    assert_eq!(port.play_calls, 0);
    assert_eq!(controller.transport_state(), TransportState::Stopped);
    assert_eq!(controller.resume_position(), Some(resume));
}

#[test]
fn hold_blocks_pause_and_stop_without_mutation() {
    let mut controller = controller();
    seat_disc_and_close(&mut controller);
    controller.request_play().unwrap();
    controller.set_hold_enabled(true);

    let pause_error = controller.request_pause().unwrap_err();
    assert_eq!(
        pause_error.kind(),
        TransportTransitionErrorKind::HoldEnabled
    );
    assert_eq!(controller.transport_state(), TransportState::Playing);

    let stop_error = controller
        .request_stop(PlaybackPosition::from_millis(12_345))
        .unwrap_err();
    assert_eq!(stop_error.kind(), TransportTransitionErrorKind::HoldEnabled);
    assert_eq!(controller.transport_state(), TransportState::Playing);
    assert_eq!(controller.resume_position(), None);
}

#[test]
fn hold_blocks_open_without_starting_lid_motion() {
    let mut controller = controller();
    controller.set_hold_enabled(true);

    let error = controller.request_lid_open().unwrap_err();

    assert!(error.hold_enabled());
    assert_eq!(error.state(), LidState::Closed);
    assert_eq!(error.transport_state(), None);
    assert_eq!(controller.lid_state(), LidState::Closed);
}

#[test]
fn disabling_hold_restores_normal_control_behavior() {
    let mut controller = controller();
    controller.set_hold_enabled(true);
    assert!(controller.request_lid_open().is_err());

    controller.set_hold_enabled(false);
    controller.request_lid_open().unwrap();

    assert_eq!(controller.lid_state(), LidState::Opening);
}

#[test]
fn hold_blocks_short_ams_without_backend_calls() {
    let mut controller = controller();
    seat_disc_and_close(&mut controller);
    controller.set_hold_enabled(true);
    let mut port = FakeNavigationPort::default();

    assert_navigation_hold(controller.request_next_with(&mut port).unwrap_err());
    assert_navigation_hold(
        controller
            .request_previous_with(&mut port, PlaybackPosition::from_millis(5_000))
            .unwrap_err(),
    );

    assert_eq!(port.next_calls, 0);
    assert_eq!(port.previous_calls, 0);
    assert!(port.seeks.is_empty());
    assert_eq!(controller.transport_state(), TransportState::Stopped);
}

#[test]
fn hold_blocks_scan_begin_and_scan_steps_but_release_can_unwind_active_scan() {
    let mut controller = controller();
    seat_disc_and_close(&mut controller);
    controller.request_play().unwrap();
    controller.set_hold_enabled(true);

    let begin_error = controller
        .request_scan_begin(ScanDirection::Forward)
        .unwrap_err();
    assert_eq!(
        begin_error.kind(),
        NavigationTransitionErrorKind::HoldEnabled
    );
    assert_eq!(controller.transport_state(), TransportState::Playing);

    controller.set_hold_enabled(false);
    controller
        .request_scan_begin(ScanDirection::Forward)
        .unwrap();
    assert_eq!(controller.transport_state(), TransportState::SeekingForward);

    controller.set_hold_enabled(true);
    let mut port = FakeNavigationPort::default();
    let seek_error = controller
        .request_scan_seek_with(
            &mut port,
            PlaybackPosition::from_millis(1_000),
            PlaybackPosition::from_millis(2_000),
        )
        .unwrap_err();
    assert_navigation_hold(seek_error);
    assert!(port.seeks.is_empty());
    assert_eq!(controller.transport_state(), TransportState::SeekingForward);

    // Releasing an already-held scan is completion/cleanup, not a new locked
    // command. It must remain legal so HOLD cannot strand intermediate state.
    controller.request_scan_end().unwrap();
    assert_eq!(controller.transport_state(), TransportState::Playing);
    assert!(controller.hold_enabled());
}

#[test]
fn hold_blocks_short_menu_cycle_but_not_automatic_track_completion_policy() {
    let mut controller = controller();
    seat_disc_and_close(&mut controller);
    controller.request_play().unwrap();

    assert_eq!(
        controller.request_cycle_play_mode().unwrap(),
        PlayMode::RepeatAll
    );
    controller.set_hold_enabled(true);

    let error = controller.request_cycle_play_mode().unwrap_err();
    assert_eq!(error.kind(), PlayModeTransitionErrorKind::HoldEnabled);
    assert_eq!(controller.play_mode(), PlayMode::RepeatAll);

    // HOLD locks user controls, not automatic playback events.
    assert_eq!(
        controller.track_completion_intent(true).unwrap(),
        TrackCompletionIntent::RestartDisc
    );
}
