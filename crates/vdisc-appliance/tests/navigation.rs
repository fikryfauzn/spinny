use vdisc_appliance::{
    De200Controller, DiscState, LidState, NavigationError, NavigationPlaybackPort,
    NavigationTransitionErrorKind, PlaybackPosition, ResumePlaybackPort, ScanDirection,
    TransportState, Volume,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BackendError {
    Seek,
    Next,
    Previous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BackendCall {
    Seek(PlaybackPosition),
    Next,
    Previous,
    Play,
}

#[derive(Debug, Default)]
struct MockPort {
    calls: Vec<BackendCall>,
    fail_seek: bool,
    fail_next: bool,
    fail_previous: bool,
}

impl NavigationPlaybackPort for MockPort {
    type Error = BackendError;

    fn seek(&mut self, position: PlaybackPosition) -> Result<(), Self::Error> {
        self.calls.push(BackendCall::Seek(position));
        if self.fail_seek {
            return Err(BackendError::Seek);
        }
        Ok(())
    }

    fn next_track(&mut self) -> Result<(), Self::Error> {
        self.calls.push(BackendCall::Next);
        if self.fail_next {
            return Err(BackendError::Next);
        }
        Ok(())
    }

    fn previous_track(&mut self) -> Result<(), Self::Error> {
        self.calls.push(BackendCall::Previous);
        if self.fail_previous {
            return Err(BackendError::Previous);
        }
        Ok(())
    }
}

impl ResumePlaybackPort for MockPort {
    type Error = BackendError;

    fn seek(&mut self, position: PlaybackPosition) -> Result<(), Self::Error> {
        <Self as NavigationPlaybackPort>::seek(self, position)
    }

    fn play(&mut self) -> Result<(), Self::Error> {
        self.calls.push(BackendCall::Play);
        Ok(())
    }
}

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

fn create_resume_then_play(controller: &mut De200Controller, port: &mut MockPort) {
    controller.request_play().unwrap();
    controller
        .request_stop(PlaybackPosition::from_millis(42_000))
        .unwrap();
    controller.request_play_with(port).unwrap();
    port.calls.clear();

    assert_eq!(controller.transport_state(), TransportState::Playing);
    assert_eq!(
        controller.resume_position(),
        Some(PlaybackPosition::from_millis(42_000))
    );
}

#[test]
fn next_ams_calls_backend_and_preserves_transport() {
    let mut controller = controller();
    seat_and_close_disc(&mut controller);
    controller.request_play().unwrap();
    let mut port = MockPort::default();

    controller.request_next_with(&mut port).unwrap();

    assert_eq!(port.calls, vec![BackendCall::Next]);
    assert_eq!(controller.transport_state(), TransportState::Playing);
}

#[test]
fn previous_ams_restarts_current_track_when_position_is_nonzero() {
    let mut controller = controller();
    seat_and_close_disc(&mut controller);
    controller.request_play().unwrap();
    let mut port = MockPort::default();

    controller
        .request_previous_with(&mut port, PlaybackPosition::from_millis(25_000))
        .unwrap();

    assert_eq!(
        port.calls,
        vec![BackendCall::Seek(PlaybackPosition::from_millis(0))]
    );
    assert_eq!(controller.transport_state(), TransportState::Playing);
}

#[test]
fn previous_ams_moves_to_previous_track_when_already_at_zero() {
    let mut controller = controller();
    seat_and_close_disc(&mut controller);
    let mut port = MockPort::default();

    controller
        .request_previous_with(&mut port, PlaybackPosition::from_millis(0))
        .unwrap();

    assert_eq!(port.calls, vec![BackendCall::Previous]);
    assert_eq!(controller.transport_state(), TransportState::Stopped);
}

#[test]
fn successful_track_navigation_clears_stale_resume_memory() {
    let mut controller = controller();
    seat_and_close_disc(&mut controller);
    controller.request_play().unwrap();
    controller
        .request_stop(PlaybackPosition::from_millis(11_000))
        .unwrap();
    let mut port = MockPort::default();

    controller.request_next_with(&mut port).unwrap();

    assert_eq!(controller.resume_position(), None);
    assert_eq!(controller.transport_state(), TransportState::Stopped);
}

#[test]
fn failed_navigation_preserves_resume_memory_and_transport() {
    let mut controller = controller();
    seat_and_close_disc(&mut controller);
    controller.request_play().unwrap();
    let remembered = PlaybackPosition::from_millis(11_000);
    controller.request_stop(remembered).unwrap();
    let mut port = MockPort {
        fail_next: true,
        ..MockPort::default()
    };

    let error = controller.request_next_with(&mut port).unwrap_err();

    assert_eq!(error, NavigationError::Backend(BackendError::Next));
    assert_eq!(controller.resume_position(), Some(remembered));
    assert_eq!(controller.transport_state(), TransportState::Stopped);
}

#[test]
fn ams_requires_closed_lid_and_seated_disc() {
    let mut controller = controller();
    let mut port = MockPort::default();

    let no_disc = controller.request_next_with(&mut port).unwrap_err();
    let NavigationError::Transition(no_disc) = no_disc else {
        panic!("expected transition error");
    };
    assert_eq!(no_disc.kind(), NavigationTransitionErrorKind::DiscNotSeated);

    controller.request_lid_open().unwrap();
    controller.notify_lid_opened().unwrap();
    controller.request_disc_insert().unwrap();
    controller.notify_disc_validation_accepted().unwrap();
    controller.notify_disc_seated().unwrap();

    let open_lid = controller.request_next_with(&mut port).unwrap_err();
    let NavigationError::Transition(open_lid) = open_lid else {
        panic!("expected transition error");
    };
    assert_eq!(open_lid.kind(), NavigationTransitionErrorKind::LidNotClosed);
    assert!(port.calls.is_empty());
}

#[test]
fn held_forward_scan_enters_seeking_executes_forward_target_and_returns_to_playing() {
    let mut controller = controller();
    seat_and_close_disc(&mut controller);
    controller.request_play().unwrap();
    let mut port = MockPort::default();

    controller
        .request_scan_begin(ScanDirection::Forward)
        .unwrap();
    assert_eq!(controller.transport_state(), TransportState::SeekingForward);

    controller
        .request_scan_seek_with(
            &mut port,
            PlaybackPosition::from_millis(10_000),
            PlaybackPosition::from_millis(12_000),
        )
        .unwrap();

    assert_eq!(
        port.calls,
        vec![BackendCall::Seek(PlaybackPosition::from_millis(12_000))]
    );

    controller.request_scan_end().unwrap();
    assert_eq!(controller.transport_state(), TransportState::Playing);
}

#[test]
fn held_backward_scan_enters_seeking_and_accepts_backward_target() {
    let mut controller = controller();
    seat_and_close_disc(&mut controller);
    controller.request_play().unwrap();
    let mut port = MockPort::default();

    controller
        .request_scan_begin(ScanDirection::Backward)
        .unwrap();
    assert_eq!(
        controller.transport_state(),
        TransportState::SeekingBackward
    );

    controller
        .request_scan_seek_with(
            &mut port,
            PlaybackPosition::from_millis(10_000),
            PlaybackPosition::from_millis(8_000),
        )
        .unwrap();

    assert_eq!(
        port.calls,
        vec![BackendCall::Seek(PlaybackPosition::from_millis(8_000))]
    );
}

#[test]
fn scan_begin_requires_active_playing_transport() {
    for paused in [false, true] {
        let mut controller = controller();
        seat_and_close_disc(&mut controller);
        if paused {
            controller.request_play().unwrap();
            controller.request_pause().unwrap();
        }

        let error = controller
            .request_scan_begin(ScanDirection::Forward)
            .unwrap_err();

        assert_eq!(
            error.kind(),
            NavigationTransitionErrorKind::InvalidTransportState
        );
        assert_eq!(
            controller.transport_state(),
            if paused {
                TransportState::Paused
            } else {
                TransportState::Stopped
            }
        );
    }
}

#[test]
fn scan_seek_rejects_target_that_moves_against_held_direction_without_backend_call() {
    let mut controller = controller();
    seat_and_close_disc(&mut controller);
    controller.request_play().unwrap();
    controller
        .request_scan_begin(ScanDirection::Forward)
        .unwrap();
    let mut port = MockPort::default();

    let error = controller
        .request_scan_seek_with(
            &mut port,
            PlaybackPosition::from_millis(10_000),
            PlaybackPosition::from_millis(9_000),
        )
        .unwrap_err();

    let NavigationError::Transition(error) = error else {
        panic!("expected transition error");
    };
    assert_eq!(
        error.kind(),
        NavigationTransitionErrorKind::WrongScanDirection
    );
    assert!(port.calls.is_empty());
    assert_eq!(controller.transport_state(), TransportState::SeekingForward);
}

#[test]
fn scan_backend_failure_keeps_seeking_state_and_old_resume_memory() {
    let mut controller = controller();
    seat_and_close_disc(&mut controller);
    let mut port = MockPort::default();
    create_resume_then_play(&mut controller, &mut port);
    controller
        .request_scan_begin(ScanDirection::Forward)
        .unwrap();
    port.fail_seek = true;

    let error = controller
        .request_scan_seek_with(
            &mut port,
            PlaybackPosition::from_millis(42_000),
            PlaybackPosition::from_millis(44_000),
        )
        .unwrap_err();

    assert_eq!(error, NavigationError::Backend(BackendError::Seek));
    assert_eq!(controller.transport_state(), TransportState::SeekingForward);
    assert_eq!(
        controller.resume_position(),
        Some(PlaybackPosition::from_millis(42_000))
    );
}

#[test]
fn successful_scan_seek_invalidates_old_stop_resume_memory() {
    let mut controller = controller();
    seat_and_close_disc(&mut controller);
    let mut port = MockPort::default();
    create_resume_then_play(&mut controller, &mut port);
    controller
        .request_scan_begin(ScanDirection::Forward)
        .unwrap();

    controller
        .request_scan_seek_with(
            &mut port,
            PlaybackPosition::from_millis(42_000),
            PlaybackPosition::from_millis(45_000),
        )
        .unwrap();

    assert_eq!(controller.resume_position(), None);
    assert_eq!(controller.transport_state(), TransportState::SeekingForward);
}

#[test]
fn scan_end_is_rejected_when_no_scan_is_active() {
    let mut controller = controller();
    seat_and_close_disc(&mut controller);
    controller.request_play().unwrap();

    let error = controller.request_scan_end().unwrap_err();

    assert_eq!(
        error.kind(),
        NavigationTransitionErrorKind::InvalidTransportState
    );
    assert_eq!(controller.transport_state(), TransportState::Playing);
}
